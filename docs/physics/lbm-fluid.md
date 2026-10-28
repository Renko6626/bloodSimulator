# 局部 LBM 流体模型与算法

2026-10-29。当前阶段选用 **D2Q9 MRT + Guo 外力、LSQ-FD 给定速度边界重建、帧末平均密度控制**。已在 `feat/lbm-lsq-integration` 分支接入默认关闭的 `lbm-gpu` 路径；实际检查与未验证项见文末。默认游戏仍使用解析流体。

## 1. 模型承担什么

全局血管图提供各支路流量，局部 48×48 世界单位移动窗口计算二维流场。窗口开口速度来自全局流量和管内剖面；窗口外仍用解析流。当前局部压力不会反向修改全局血管阻力或流量。

局部求解的是等温、弱可压缩的二维黏性流体。连续极限的目标方程为

\[
\partial_t\rho+\nabla\cdot(\rho\mathbf u)=0,\qquad
\partial_t(\rho\mathbf u)+\nabla\cdot(\rho\mathbf u\mathbf u)
=-\nabla p+\nabla\cdot\boldsymbol\sigma+\mathbf F.
\]

这是低马赫数近似；数值密度允许小幅变化，用于携带压力。帧末控制会额外改变质量与动量，因此整套离散算法不能称为严格局部守恒求解器。

全局体积流量通过等效深度 `D=1` 转成平面通量。当前管内背景剖面为二维抛物线 `u(r)=3Q[1−(r/R)²]/(4RD)`。它与三维圆管模型不同，世界长度、时间和参考密度也没有在这里自动标定为 SI 单位。

## 2. 格子、单位和分布函数

Fine 使用 192² 格子、间距 `h=0.25`；Coarse 使用 96²、`h=0.5`。格子时间步记作 `Δt`，一次游戏帧包含多个机械子步，每个机械子步又包含多个格子步。

\[
\mathbf u_{lu}=\mathbf u_{world}\frac{\Delta t}{h},\qquad
\nu_{lu}=\nu_{world}\frac{\Delta t}{h^2},\qquad
\tau=\frac12+3\nu_{lu},\quad s_\nu=1/\tau.
\]

D2Q9 速度依次为 `(0,0),(1,0),(0,1),(−1,0),(0,−1),(1,1),(−1,1),(−1,−1),(1,−1)`；权重依次为 `4/9`、四个 `1/9`、四个 `1/36`，声速平方 `c_s²=1/3`。下文未特别注明时均使用格子单位。

GPU 保存偏移分布 `g_q=f_q−w_q`，减少参考密度为 1 时的浮点消减误差。宏观量为

\[
\rho=1+\sum_qg_q,\qquad \mathbf j=\sum_q\mathbf c_qg_q,
\qquad \mathbf u=\frac{\mathbf j+\mathbf F/2}{\rho}.
\]

压力满足 `p=c_s²ρ`；常用的相对压力为 `p′=c_s²(ρ−1)`。换回世界单位的压力尺度还包含参考密度与 `(h/Δt)²`，不能直接把数值 `ρ−1` 当作血压读数。

偏移平衡分布为

\[
g_q^{eq}=w_q\left[(\rho-1)+\rho\left(3\mathbf c_q\cdot\mathbf u+
\frac92(\mathbf c_q\cdot\mathbf u)^2-\frac32|\mathbf u|^2\right)\right].
\]

## 3. MRT 碰撞、外力和迁移

使用正交原始矩基 `ρ,e,ε,jx,qx,jy,qy,pxx,pxy`，松弛率为

\[
S=\operatorname{diag}(0,1.1,1,0,1.2,0,1.2,s_\nu,s_\nu).
\]

每格碰撞为

\[
g^*=g-M^{-1}SM(g-g^{eq})+M^{-1}(I-S/2)M\Phi,
\]

其中 Guo 外力项为

\[
\Phi_q=w_q\left[3(\mathbf c_q-\mathbf u)+9(\mathbf c_q\cdot\mathbf u)\mathbf c_q\right]\cdot\mathbf F.
\]

剪切黏度由 `sν` 决定，迹相关松弛率固定为 `1.1`。这两者在边界应力重建中分别使用，不能将该 MRT 实现直接写成单一 BGK 松弛时间。

碰撞后沿九个方向 push streaming。固体链路使用 halfway bounce-back；非开口的窗口边缘也反弹，开口随后重建。正式移动窗口没有周期绕回。栅格化管壁具有阶梯几何误差，本轮没有引入曲面插值反弹。

`StepInput.acceleration` 当前换算为 `F=a Δt²/h`，没有再逐格乘以 `ρ`。严格说，这是按参考密度归一的力密度接口；当 `ρ` 偏离 1 时，不应把它解释为精确的逐格恒定加速度 `F=ρa`。机械耦合主要通过已有的 `apply_kicks` 交换冲量，后续格子推进的该外力参数通常为零。

MRT 与 Guo 方法分别参考 [Lallemand 与 Luo，2000](https://doi.org/10.1103/PhysRevE.61.6546) 和 [Guo、Zheng 与 Shi，2002](https://doi.org/10.1103/PhysRevE.65.046308)。具体松弛率和实现适配以本项目代码为准。

## 4. 给定速度为什么仍会出现密度漂移

每个开口指定完整速度向量，既保留沿管方向，也保留其相对窗口边界的切向分量。离散剖面按 `D h Σ(u·n_out)=Q_out` 归一，使给定的体积流量符合全局图。斜管因此无需与窗口边缘垂直。

令 `n` 为指向窗口内部的轴向单位法线，`t=(−ny,nx)`，`un=u_b·n`。从 streaming 后尚可用的分布计算

\[
A=\sum_{\mathbf c_q\cdot\mathbf n=0}f_q,\qquad
B=\sum_{\mathbf c_q\cdot\mathbf n=-1}f_q,
\qquad \rho_b=\frac{A+2B-F_n/2}{1-u_n}.
\]

因此开口规定的是速度，密度由相邻分布及其历史闭合。不同开口可以具有不同密度。即使体积通量 `ΣQ=0`，质量通量 `ΣρQ` 也不必为零；全九分布重建还会改变边界节点的暂存质量。低马赫数并不保证这些差额长期累计为零。

[短程质量收支诊断](../research/2026-10-29-lbm-mass-budget/README.md)观察到体积流入/流出接近相等，而密度加权通量及重建项产生净质量增加。它支持“开放速度边界的质量预算持续有偏差”，尚不足以把全部漂移归因于一个单独的密度差或某个浮点舍入项。同行也讨论过速度边界下的质量增长，见 [Krüger 等的研究](https://arxiv.org/abs/0812.3242)。

## 5. LSQ-FD 边界重建

原边界只补缺失分布，在当前低黏度配置中出现增长的边界模。选定方案改为：从邻域速度估计梯度，由梯度构造非平衡应力，再重建开口节点的全部九个分布。

### 5.1 用可见邻域拟合梯度

边界目标速度 `u_b` 固定，在半径 4 格的圆盘内取内部流体速度和同侧开口的目标速度。边界样本只读不可变目标 buffer，避免同一 GPU pass 读取正在改写的边界分布。以 cell-center 连线的 supercover 检查可见性；连线经过固体或擦过固体角点的候选被排除。

对每个速度分量拟合

\[
u_j-u_b=a s_j+b z_j+\frac12c s_j^2+d s_jz_j+\frac12e z_j^2,
\qquad (s_j,z_j)=(\delta_j\cdot n,\delta_j\cdot t)/4.
\]

目标函数的距离权为 `1/(1+|δj|²)`；使用加权矩阵的 SVD，要求 `rank=5`、相对奇异值截断 `1e−12`、条件数不超过 `1000`。由此得到 `∂n u=a/4`、`∂t u=b/4`，再转换成 `∂x u, ∂y u`。

导数权重只依赖几何，可以在创建窗口和移窗时预计算，速度更新时复用。实验曾用 Python/NumPy 生成文件；正式模块用 Rust/faer 在模块内生成，不需要几何专用离线文件。采样不足或矩阵退化时返回不支持，不自动扩大半径、降低阶数或钳制梯度。

### 5.2 从梯度重建非平衡应力

记 `d=∂x ux+∂y uy`，`e=∂x ux−∂y uy`，`se=1.1`。重建采用

\[
\begin{aligned}
P_{xx}&=-\frac{\rho_b}{3}(d/s_e+e/s_\nu)-u_xF_x,\\
P_{yy}&=-\frac{\rho_b}{3}(d/s_e-e/s_\nu)-u_yF_y,\\
P_{xy}&=-\frac{\rho_b}{3s_\nu}(\partial_yu_x+\partial_xu_y)
-\frac12(u_xF_y+u_yF_x).
\end{aligned}
\]

全部九个偏移分布更新为

\[
g_q=g_q^{eq}(\rho_b,u_b)+\frac92w_q
\left[(c_{qx}^2-1/3)P_{xx}+(c_{qy}^2-1/3)P_{yy}+2c_{qx}c_{qy}P_{xy}\right]
-\frac32w_q\mathbf c_q\cdot\mathbf F.
\]

应力项的零阶、一阶矩为零，最后一项提供 Guo 的半步力修正，因而重建符合目标 `ρ_b,u_b`。边界保留由梯度估计的非平衡二阶信息；未保留的高阶非平衡模被滤去。它与“直接把分布重置成平衡态”、从内点复制全部非平衡分布的 NEQ 外推是不同算法；当前采用 FD 路线，NEQ 仍是备选。

FD 全分布重建参考 [Latt 等，2008](https://doi.org/10.1103/PhysRevE.77.056703)。有限邻域二次拟合、MRT 迹/剪切分离和本项目外力适配属于具体实现选择，文献不能代替其稳定性与精度验证。详见 [LSQ 候选记录](../research/2026-10-28-lbm-fd-boundary/lsq-candidate/README.md)。

## 6. 帧末平均密度控制

每个游戏帧完成后，对当前窗口内 `N` 个非固体格计算

\[
\bar\rho=1+\frac1N\sum_{i,q}g_{iq},\qquad \delta=\bar\rho-1.
\]

CPU 对 GPU 原始偏移分布使用 f64 求和；单独 GPU pass 对每格加一个平衡分布差：

\[
\begin{aligned}
g'_{iq}&=g_{iq}+f_q^{eq}(\rho_i-\delta,u_i)-f_q^{eq}(\rho_i,u_i)\\
&=g_{iq}-\delta w_q[1+3\mathbf c_q\cdot\mathbf u_i+
\tfrac92(\mathbf c_q\cdot\mathbf u_i)^2-\tfrac32|\mathbf u_i|^2].
\end{aligned}
\]

这里的 `u_i` 包含 Guo 半步力，修正期间 `F` 保持不变。理想代数下，密度统一减去 `δ`，原速度、密度空间差，以及相对新平衡态的非平衡残差均保持不变。含力时可观测的非平衡应力使用

\[
\Pi^{neq}=\sum_q\mathbf c_q\mathbf c_qf_q-\rho\mathbf u\mathbf u-c_s^2\rho I
+(\mathbf u\mathbf F+\mathbf F\mathbf u)/2.
\]

这项修正借鉴 [OpenLB TR3](https://www.openlb.net/wp-content/uploads/2011/12/olb-tr3.pdf) 的均值偏移思路，保留本项目 MRT 碰撞与独立帧尾调度。没有改成 BGK、单改 `f0` 或按比例缩放全部分布。

其物理代价必须单独记账：

\[
\Delta M_{lu}=-N\delta,\qquad
\Delta\mathbf P_{lu}=-\delta\sum_i\mathbf u_i.
\]

质量的世界平面积分还需乘 `h²D`。这里是数值补偿；推算的请求补偿量与实际 GPU 修正后测得的量须区分。它不会自动成为细胞或机器人的反冲量。虽然速度瞬时不变，总流体动量仍会改变。

均值控制同时滤掉平均密度的周期起伏和长期漂移。空间均匀压力偏移不直接改变当时的压力梯度，但未来演化、绝对压力响应和密度参与的耦合可能受影响。瞬时非平衡矩保持不意味着长期应力场完全相同。修正后的密度必须为正；实现不应以静默 clamp 隐藏失效。

## 7. 帧循环、移动窗口与机械耦合

正式接入的时序为：

1. 每帧更新全局背景；每个机械子步继续 `prepare`，必要时移窗并更新开口剖面。
2. 读取当前缓存流场，进行实体/流体冲量交换；推进该子步需要的格子步。
3. 中间子步读回流场供下一子步使用。最后一个流体 batch 延迟读回，机械子步完成后调用 `finish_frame`。
4. `finish_frame` 执行一次均值控制，再读回并验证流场、更新共享采样缓存。

移窗时复制重叠区域的原始分布，新条带用背景平衡态初始化。新几何必须重新生成 LSQ 权重；原始重叠复制的验证应在帧尾控制之前进行，因为之后的统一密度偏移会改变重叠区分布。

现有实体耦合使用有限阻力 marker，插值和反向分配共享支撑权重，单次冲量交换有对应反作用。它还不等同于严格浸入边界无滑移或完全阻塞。旧冲量测试不能单独证明加入均值控制后的流体与实体总动量守恒。

当前时间步选择考虑机械子步、黏度与参考 lattice 速度上限，再向上取整格子步数。这是配置规则，不能作为整个运行中最大速度始终低于上限的证明。

## 8. 适用范围与已有证据

LSQ 缓解了斜管壁附近轴向差分缺少流体采样的问题，但不保证任意斜率、任意宽度都受支持。当前 rasterizer 仍要求每个开口穿过独立直管段，避开窗口角点、弯曲段和支路歧义，管方向与边界法线的点积绝对值至少 `0.1`；LSQ 还必须满足秩和条件数要求。分叉可以在窗口内部，窗口边界直接切过分叉仍不在支持范围。默认 `small_world` 的已有不支持结论未撤销。

| 证据 | 能支持的结论 | 限制 |
| --- | --- | --- |
| 45°及两个选定分叉，Fine/Coarse 共五例各 160 帧 | 选定斜开口可以生成有效采样并完成运行 | 本机 llvmpipe；不覆盖任意几何 |
| 未控制 LSQ 单例 60000 帧 | 该例没有数值爆炸 | 周期均值持续增长，仍有质量漂移 |
| RTX 3060，fork_a Fine，控制/未控制各 10000 帧 | 已选控制方案在该例持续运行并抑制均值偏移 | 固定几何、无机械，基线不是真解 |

[10000 帧结果](../research/2026-10-29-lbm-budget-longrun/README.md)：未控制末平均密度 `1.0073113874`；控制后全程最大平均密度残差约 `2.12×10⁻¹⁰`。控制与基线的速度相对 L2 差峰值为全域 `0.03459%`、中心 `0.05510%`，去均值密度与非平衡应力差峰值约 `0.748%`、`0.735%`。这些是两种数值解的差异，不是相对真实流场的误差；去均值密度比较也排除了被控制的全局压力模。

两组最大 lattice 速度约 `0.06831`，高于原 `0.05` 参考值。实卡结果支持阶段性采用，尚未完成网格收敛、解析/实验真解精度、完整机械耦合、移窗长期行为与绝对压力响应验收。

同卡流体预算中，控制路径平均 `1.898 ms/帧`、p95 `2.098 ms/帧`；未控制为 `1.475/1.637 ms`。这是 192²、14 个格子步/帧、两次 field 加一次 raw 读回的无机械单窗口预算，不包含全局图求解、迁移、Rapier 或渲染。基准每帧更新一次剖面，正式路径每机械子步更新，不能直接承诺相同完整游戏帧耗时。当前采用 CPU 求均值与原始分布读回，尚未做 GPU reduction 优化。

## 9. 实现与验证状态

算法实验证据来自 [均值控制研究](../research/2026-10-29-lbm-mean-density/README.md) 和上述长测。正式化已在 `feat/lbm-lsq-integration` 实现，依据 [已确认方案](../superpowers/specs/2026-10-29-lbm-integration-preparation.md) 与 [实施记录](../superpowers/plans/2026-10-29-lbm-integration.md)。本机相关 CPU 与软件 GPU 功能检查覆盖权重、边界矩、均值修正、缓存、帧尾调度和窗口迁移；simulation 与 Godot bridge 的 feature 开/关编译、格式检查通过。具体命令见实施记录。本轮没有重跑 RTX 长测、完整 Godot 游戏或 UI 交互；本机功能检查不替代这些验收。

正式职责：[stencil.rs](../../crates/fluid-gpu/src/lbm/stencil.rs) 生成权重，[lbm.wgsl](../../crates/fluid-gpu/src/lbm/lbm.wgsl) 与 [gpu.rs](../../crates/fluid-gpu/src/lbm/gpu.rs) 执行数值算法；[streaming_window.rs](../../crates/simulation/src/fluid/streaming_window.rs) 管理窗口与缓存，[lbm_coupling.rs](../../crates/simulation/src/fluid/lbm_coupling.rs) 管理机械交换，[runtime.rs](../../crates/simulation/src/runtime.rs) 安排帧尾调用。研究目录保留冻结源码与数据，不能把其中的诊断开关或离线权重文件当作正式运行依赖。


后续已完成 [LBM 游戏验证场实测](../research/2026-10-29-lbm-gameplay/README.md)：现有机械耦合接入72个真实红细胞与机器人，固定/移动各60秒测量、移动5次移窗，运行守卫通过；所记录Rust与Godot状态在f32精度下一致。这补充了选定场景的机械运行证据，不是严格封堵、动量守恒或广泛精度验收。Godot/Xorg整体帧间隔p95约70–71ms，性能尚未达60FPS；原2.10ms纯流体预算不能外推为游戏帧率。
