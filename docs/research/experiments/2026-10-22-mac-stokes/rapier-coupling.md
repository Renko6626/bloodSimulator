# Rapier 0.36.1 软体与 MAC/Stokes 最小耦合接口核查

日期：2026-10-22。只读代码研究；未实现算法，未改产品文件。这里的可行性指能从外部交换有限阻力/冲量，并不等于已经实现无滑移或不可渗透边界。

## 可直接使用的状态与作用入口

- 项目 `crates/simulation/src/cell.rs:4-16` 用 `SoftBodyBuilder::disk(..., cell_vertices).mass(1.0)` 建细胞。默认 `cell_vertices=12`、`cell_radius=0.7`、`cell_drag=20`（`config.rs:23-31`）。Rapier `soft_body_builder_shapes.rs:554-565` 说明 disk 是仅含指定数量边界粒子的多边形；这里确是 12 个机械节点，没有额外中心质量节点。Rapier `soft_body_builder_settings.rs:38-42` 的 `.mass(1.0)` 是**软体总标称质量**，均分后每节点约 `1/12`；`.particle_mass` 与 `.masses` 分别支持统一节点质量和逐节点质量（同文件 `31-49`）。运行时 `SoftBody::mass()` 求和（`soft_body_accessors.rs:262-265`），`body.particles()[i].mass()/inv_mass()/force()` 可读（`soft_body_accessors.rs:21-29`；`soft_body_elements.rs:80-112`）。当前无公开的运行时逐粒子改质量 setter；建造时配置质量。
- `SoftBody::particle_position(i)`、`particle_velocity(i)`、`particle_positions()`、`particle_velocities()` 与 `set_particle_velocity(i, v)` 可直接复用（Rapier `soft_body_accessors.rs:31-67`）；项目现有 `cell::advect` 就调用这些入口（`cell.rs:19-33`）。不要用 `set_particle_position` 充当力学耦合：它是位置传送（Rapier `soft_body_accessors.rs:51-59`）。边界拓扑可由 `body.boundary()` 获取（同文件 `99-104`），若将来拓扑可能改变，可观察 `topology_version()`（同文件 `13-19`）。
- `SoftBody::add_particle_force(i,F,wake_up)`、`reset_forces` 在 Rapier `soft_body_motion.rs:98-115`，节点用户力**持续**到显式清零；内部求解把它按 `(F/m)dt` 加入速度（`soft_constraint_prepare.rs:279-303`）。`apply_particle_impulse(i,J,wake_up)` 在 `soft_body_motion.rs:130-137`，即时 `Δv=J/m`，适合一次子步的外部交换。`apply_impulse`（`117-128`）对所有自由节点给同一**速度改变量**，参数虽名 impulse，但不是“总冲量均分”；精确回传应逐节点调用。钉住节点的逆质量为零，无法按通常自由节点质量交换（`soft_body_motion.rs:33-47,130-137`）。
- `PhysicsWorld.soft_bodies` 与 `integration_parameters` 公开（Rapier `physics_world.rs:64-90`），`world.step()` 是公开的整步入口（同文件 `126-132`）。软体自身保留约束、面积保持、接触和位置积分；内部求解器并无向外暴露“给定边界力得到含接触/弹性约束的速度响应”算子。`soft_body.rs:23-25` 还明确隐藏的 root rigid body 不应被移动、删除或接 joint；不能把它当作流固耦合代理。

上面的 Rapier 文件均位于本机 `/data/sunyunbo/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/rapier2d-0.36.1/src/dynamics/`，其中 `soft_body_*` 归在 `soft_body/`；`physics_world.rs` 位于 `src/pipeline/`，`soft_constraint_prepare.rs` 位于 `src/dynamics/solver/soft_constraint/soft_constraints_set/`。

## 当前调用顺序与可插入时点

`runtime.rs:247-274`：每固定步先算脉动压力并 `environment.update(pressure)` 一次；设置 `dt=config.dt/config.substeps`；每个子步依次给机器人取流速并 `robot::drive`，对每个细胞调用 `cell::advect`，然后调用 `physics.step()`。`cell::advect` 当前令各节点 `v←v+(u-v)(1-exp(-drag·dt))`（`cell.rs:25-31`），只改固体速度，没有向流体写反作用。这段旧单向松弛应在耦合实验场景中替换为交换量，不能和新阻力叠加。

最小双向实验可在每次 `physics.step()` **之前**读当前 12 节点的 `X_i,V_i`，构造无质量边界标记，采样当前网格速度，算界面阻力，向网格散布反作用，再解本子步准稳态 Stokes 并把最终界面力换成 `J_i=F_i dt` 调 `apply_particle_impulse`，最后只调用一次 `physics.step()`。若阻力依赖解后的网格速度，需做明确的弱固定点迭代/线性外迭代或选已知场作预测；不能以“先算 F、同时用 F 解 u”而无迭代构成闭合求解。对于首个 A 阶段固定通道，无细胞也无此接口需求。`Environment::velocity_at`（`runtime.rs:36-44`）可承接日后网格取样；`Environment::update`（`46-50`）当前只更新 0D 网络，局部流体推进必须有其自己的子步时机。`snapshot` 只读节点位置和流速展示（`runtime.rs:279-315`），不应承担物理推进。

`add_particle_force` 也可用，但必须处理它跨子步持久化，且 `reset_forces` 清除该软体所有用户力，可能误清其他子系统的力。冲量入口不需要新状态或清零流程，作为最小实验更直接。无论哪个入口，流体网格与固体都用同一 `dt` 和同一约定的物理力/二维每单位深度单位。

## 12 点机械节点与更密标记的最小交换

对边 `(i,i+1)` 取插值参数 `a∈[0,1]`，标记 `X_q=(1-a)X_i+aX_{i+1}`，`V_q=(1-a)V_i+aV_{i+1}`，积分权重 `Δs_q` 来自边弧长分段。保留这 12 个 Rapier 节点；标记没有独立质量、弹簧或位置积分。若格面速度自由度记 `u_g`，同一核权重 `W_{qg}` 形成 `U_q=Σ_g W_{qg}u_g`。弱惩罚力（作用于固体）为 `F_q=κ Δs_q(U_q−V_q)`；传回节点 `F_i += (1-a)F_q`、`F_{i+1} += aF_q`。网格收到 `f_g += -Σ_q W_{qg}F_q/ΔA_g`（需按 MAC 的 u/v 格面控制体积分别定义）。如 `Σ_g W_{qg}=1`，节点总力与网格积分总力反向相等。冲量 `J_i=dt F_i`。用相同权重插值与散布，交换功率满足 `Σ_i F_i·V_i + Σ_g ΔA_g f_g·u_g = Σ_q F_q·(V_q−U_q) = -κΣ_q Δs_q|U_q−V_q|² ≤ 0`，这是阻力耦合的关键符号/能量检查。准稳态 Stokes 没有流体动量随时间储存；这里的网格项是瞬时功率和应力/黏性耗散平衡，不能称作真实流体动能变化。其它外部压力/端口功率、Rapier 弹性及接触功要单列，不能要求整个体系能量只下降。

有限 `κ` 允许滑移，12 节点加密标记也不保证不漏流。若要严格无滑移，需要同时求界面乘子、不可压速度/压力及 Rapier 包含质量、弹性、面积约束和接触的离散速度响应；上述公开 API 仅允许完整 `step()` 前后施力/读状态，没有暴露该响应算子或内部线性化矩阵。强耦合需要改 Rapier 接口或外层多次回滚/迭代固体状态，属于新的架构与成本决策，不能作为现有 API 已支持的简单接线。

质量一致性仍是独立问题：当前 `mass=1` 是游戏参数。若 MAC 网格把细胞内部也当具有密度 `ρ` 的流体推进，而 Rapier 节点再承担完整细胞质量，同一区域惯性可能双算，排液/浮力也不对应。准稳态 Stokes 算子无 `ρ∂_t u`，所以不应虚构网格流体动量库存；但在过渡到非定常流体、解释固体惯性或用内外全域网格时，必须明确内部流体是否被屏蔽、固体质量是总质量还是相对排液的额外质量，并核对力和二维深度单位。不要直接把节点质量设成生理值而保留全域流体惯性。

## 下一步限定实验

先按已建议的正规小通道做周期 x、上下 no-slip y 的 MAC/Stokes 核心，验证剖面、散度/流量收支、残差与耗时，不接 Rapier。随后用规定运动边界测插值/散布的力符号和功率恒等式。只有这些通过，再单独接一个现有 12 节点 Rapier 细胞，以单一子步时钟尝试有限阻力并量界面滑移、面积漂移、漏流和整步耗时。若有明确无滑移要求，先确认是否愿意扩展 Rapier 求解接口。当前资料不支持承诺整固定步 5 ms。

## 附：圆形区域的隐式体积惩罚小实验（设计建议，未实施）

**结论：值得在正规小通道 MAC/Schur 核心通过后，作为同一小域的下一项短实验；不要与核心求解器首轮调试绑在一起。** 只用固定圆体与给定平移速度的圆体、少量 κ 值，可回答“现有网格/块求解器能否处理空间变化的零阶阻力项，能否生成合理绕流和一致的体积反力，以及误差/迭代成本随 κ 如何变化”。它不改变正式细胞材料、Rapier 边界或任何产品接口。

写 `-μ∇²u+∇p+κχ(u−V)=f, ∇·u=0`，即题给 `(A+κχ)u+Gp=f+κχV`；`χ=1` 在圆内、圆外为零，MAC 的两个速度分量按各自格面/控制体积构造掩膜。这里 `κχ(V−u)` 是**流体受到的体积力密度**，并非表面牵引；对规定运动圆体的近似反力是 `F_body≈∑_g κχ_g(u_g−V_g)ΔA_g`，二维按单位厚度计。力矩同理乘 `(x_g−X_c)×`。这是 Brinkman/Darcy 型体积惩罚：原始 [Angot、Bruneau、Fabrie 1999](https://doi.org/10.1007/s002110050401) 建立障碍物惩罚的收敛/误差分析，[Carbou、Fabrie 2003](https://www.math.u-bordeaux.fr/~carbou/publi/A10.pdf) 研究粘性不可压流的边界层；[Nguyen van Yen 等，2012](https://arxiv.org/abs/1206.0002) 直接分析 Stokes 算子的有限惩罚残余滑移和离散误差平衡。因此 κ 越大并不意味着在固定 h 上无限接近无滑移。

在稳态 Stokes 且 `A≈−μΔ` 时用 `λ=κR²/μ` 或惩罚长度 `ℓ_p=√(μ/κ)`，汇报 `ℓ_p/R` 与 `ℓ_p/h`，比只列 κ 更可比。`ℓ_p` 的量级来自圆内粘性扩散与体积阻力平衡；文献的归一化 Stokes 情形残余滑移长度量级为 `√η`，本式中对应 `η=μ/κ`。建议固定 R/h 与同一几何，选 `ℓ_p/h` 从数个网格宽度减到约一个网格宽度、再到亚网格作警示点；不要预设越小越好。增大 κ 同时会改变速度块和 Schur 条件，应记录实际求解收敛。

最少观察三个量：

1. 圆周插值速度相对 V 的切向/法向 RMS 或最大误差；另看 `∮| (u−V)·n |ds`，因为带符号净通量可能相互抵消，不能单独代表局部不渗透。
2. 体积反力 `F_body` 的方向和大小，固定圆体与给定平移圆体均做同一小域、同一边界条件下的 h/κ 对比；同时查看外部流线及近圆速度。有限周期通道不能拿无限域三维球 Stokes 阻力公式作数值真值。需要可信力值时再用网格加密或独立同域参考解。
3. 整个离散块系统真实残差、离散散度/流量收支、迭代次数与耗时随 `ℓ_p/h` 的变化，判断惩罚逼近和 Schur 预条件是否同时可用。

静止圆体检验几何掩膜、速度抑制与绕流；规定平移圆体检验非零目标速度的右端、反力符号与即时 Stokes 响应。每个平移位置是单独的**准稳态快照**，不会验证真实加速度、流体动量记忆或运动历史。体积惩罚把整个圆盘当低渗透刚性区域，和现有“12 节点、可形变、内外流体可能连通的薄膜”是不同模型；圆体表现良好，不能宣称已验证薄膜 IB 的标记插值/散布伴随性、漏流、节点受力分配，也不能宣称已验证 Rapier 弹性、面积约束、接触、质量/排液或双向稳定性。若未来将圆体体积惩罚正式用于细胞，会改变其物理边界与内部流体处理，需另作路线确认。
