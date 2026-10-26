# 点力欠松弛：独立 Stokes 求解器与冻结诊断

2026-10-26。从上一轮 [hotpaths solver](../../2026-10-26-stokes-hotpaths/solver/README.md) 复制为新研究 crate `stokes_relaxation_solver`；旧源码、正式游戏和依赖均未修改。仍是 64² 周期 x、固壁 y 的 MAC/Stokes，μ=1、每细胞四点总阻力 Γ=0.25。没有流体时间惯性或严格膜封堵。Rayon 固定线程池、稳态求解、Rapier 反力方向均沿用旧实验。

## 唯一新增耦合方法

`solve_relaxed(markers, background_force, prior, omega)` 仅接受 `0 < omega <= 1`：

```text
F_trial = R (V_current - J_current u_prior)
F_new   = (1 - omega) F_old + omega F_trial
u_new   = K (f_background + S_current F_new)
```

`prior=None` 时先解无细胞背景流场，历史力为零，随后解一次施力后的完整 Stokes 场；这次调用计 2 次流体解。正常连续调用的 `prior=Some` 使用上一子步场和 `prior.marker_forces`，只做 1 次完整流体解。`F_new` 同时用于网格散布与 `Solution.marker_forces`，调用方给 Rapier 的反冲量应为 `−F_new Δt`。`omega=1` 直接调用旧 `solve_lagged`，同输入场与点力逐 bit 相同；原 `solve` 自洽路径仍保留。欠松弛是带历史的近似反馈，最终场一般不满足瞬时阻力律。

`prior=Some` 时严格核对 `marker_forces.len()==markers.len()`，网格尺寸仍按旧路径检查。**标记身份以数组索引对应**；Marker 没有 ID，移动坐标本身不足以识别同长度数组被换序，调用方必须保持每细胞四点的索引不变。实验中对象数固定。冻结流场种子原本没有这些细胞的历史力，probe 显式填入等长的零点力数组，没有将背景或局部种子当成旧细胞力。

`Solution` 原字段保留，新增：

- `force_law_max_component_abs`：最终场下所有点、x/y 分量的 `|F_i/drag_i + J_i u_new − V_i|` 最大值，单位是速度。原 `force_law_rms` 仍是 2m 个分量的 RMS。
- `exchange_power`：最终 `Σ F_new·(Ju_new−V)`；欠松弛路径可以为正，不能将其视作严格耗散保证。
- `point_force_update_ms`：从既有流速采样结束后，到点力 Vec 分配及 trial/混合循环结束的耗时；不含采样、散布或流体解。自洽 CG 路径填 0，避免冒充其不同的力迭代成本。这个单次埋点有两次时钟读取，不应从跨版本总耗时直接相减解释为净成本。

## 冻结几何原始探针

[`results/frozen.csv`](results/frozen.csv) 使用此前 16 与 64 细胞动态轨迹第 1 帧真实轮廓生成固定四点位置，所有点速度设零，撤去扰动源，不运行 Rapier、接触或机械推进。每组连续 32 次更新：先用合法无细胞泊肃叶流场作扰动；64 细胞再加一组由单个临时局部点力产生的不同空间扰动，随后也撤去。两个种子的当前细胞历史点力均显式初始化为零。每步有完整流体解、最终 RMS/最大分量残差、交换功率、网格/点力平衡与散度/动量诊断。

| 冻结布局与种子 | ω=1 第32步范数 / 初始 | ω=0.5 | ω=0.3 |
| --- | ---: | ---: | ---: |
| 16 细胞，泊肃叶 | `6.36e-11` | `3.66e-14` | `4.16e-9` |
| 64 细胞，泊肃叶 | `2.11e8` | `1.10e-12` | `2.22e-9` |
| 64 细胞，局部点力 | `2.07e8` | `1.24e-12` | `1.36e-8` |

ω=1 的放大是保留的负例；32 步全数组逐 bit 等于旧 `solve_lagged`，不是测试失败。ω=0.5/0.3 在这些固定几何和两种扰动下衰减，**不能保证移动细胞、接触或其他扰动下稳定**。ω=0.3 在这两种布局收敛比 0.5 慢；单个末态范数很小也不代表各子步阻力律闭合。probe 独立按返回的点力/采样速度复算 RMS、最大分量和功率，核对网格/点力合力；还验证首步 `prior=None` 的零历史力、错误历史长度和无效 ω 拒绝。每个有 prior 的更新均报告 `fluid_calls=1`。

历史 [16/64 冻结结果](../../2026-10-26-stokes-64-cells/frozen-results/) 的前 16 次 ω=1 泊肃叶更新，速度范数、范数比、平均 x 速度与阻力律 RMS 的已保存十进制字符串，也与本次逐行一致。另运行了复用的 [`numerical_check`](results/numerical_check.csv)，32/64/128² 的原自洽解在串行/Rayon1/2/4 间仍保持逐 bit 一致；这些检查不代替动态稳定性证据。

## 复现

在仓库根目录运行。编译复用 `/tmp/sunyunbo/blood-stokes-rayon-target`；冻结诊断短跑按本轮统一锁和 CPU2–5 执行，**不作为正式性能计时**。

```bash
cargo build --release --offline --manifest-path docs/research/experiments/2026-10-26-stokes-relaxation/solver/Cargo.toml --target-dir /tmp/sunyunbo/blood-stokes-rayon-target --bin frozen_probe
flock /tmp/blood-stokes-feedback-bench.lock taskset -c 2-5 /tmp/sunyunbo/blood-stokes-rayon-target/release/frozen_probe docs/research/experiments/2026-10-26-stokes-hotpaths/results/optimized-n64-t4-r1/outlines.csv docs/research/experiments/2026-10-26-stokes-64-cells/reference-results/optimized-t4-r1/outlines.csv 32 > docs/research/experiments/2026-10-26-stokes-relaxation/solver/results/frozen.csv
cargo build --release --offline --manifest-path docs/research/experiments/2026-10-26-stokes-relaxation/solver/Cargo.toml --target-dir /tmp/sunyunbo/blood-stokes-rayon-target --bin numerical_check
flock /tmp/blood-stokes-feedback-bench.lock taskset -c 2-5 /tmp/sunyunbo/blood-stokes-rayon-target/release/numerical_check > docs/research/experiments/2026-10-26-stokes-relaxation/solver/results/numerical_check.csv
cargo fmt --check --manifest-path docs/research/experiments/2026-10-26-stokes-relaxation/solver/Cargo.toml
cargo check --offline --manifest-path docs/research/experiments/2026-10-26-stokes-relaxation/solver/Cargo.toml --target-dir /tmp/sunyunbo/blood-stokes-rayon-target
git diff --check
```

正式动态性能与稳定性由本轮 dynamics harness 单独计时。本 probe 不回答整步能否低于 5 ms。
