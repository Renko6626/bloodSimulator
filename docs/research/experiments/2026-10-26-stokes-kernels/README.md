# Stokes 压力初值与内核优化实验

2026-10-26。沿用已确认的 Stokes 点力欠松弛、Rapier 并行和接触距离 0.002。本目录复制求解器与动态探针，不改旧归档或正式游戏；只依赖既有 Rapier 0.36.1 和 Rayon 1.12.0。

## 实现与开关

`solver/` 从欠松弛求解器复制，保留同一周期直管 MAC 离散、f64、压力 CG 容差、每子步一次完整 Stokes 更新和全部数值诊断。

- `warm-pressure`：复用已有 `Solution.pressure`，交给原本支持初值的压力 CG。初次求解仍从零开始，每步都重新计算初始残差；不新增历史状态或跳过收敛检查。
- `reciprocal`：Thomas 的分母在初始化时先完成原来的分解，再存为倒数；前代用乘法代替反复除法，不改变回代与边界。
- `fft-slices`：同一 radix-2 FFT，蝶形读取左右不相交切片，保持复数运算顺序。没有引入 FFT 库、改变变换长度或使用 unsafe。

动态探针默认启用 Rapier `parallel` 和 `warm-pressure`。组合内核候选使用 `--features reciprocal,fft-slices`；保留独立开关供对照，运行脚本总是显式指定 features。流体与 Rapier 各四线程、先后执行。细胞材料、机械迭代和阻力未改。

## 复现

在仓库根目录执行：

```bash
# 六条完整轨迹和两条独立 profiling 轨迹
python3 docs/research/experiments/2026-10-26-stokes-kernels/run.py
python3 docs/research/experiments/2026-10-26-stokes-kernels/analyze.py

# 跨旧求解器的数值对照：六组自洽解和三十六组连续欠松弛更新
cargo run --release --offline --locked --manifest-path docs/research/experiments/2026-10-26-stokes-kernels/solver/Cargo.toml --target-dir /tmp/sunyunbo/blood-stokes-rayon-target --features comparison,reciprocal,fft-slices --bin compare

# 直接运行组合候选；默认六秒，写入临时目录
cargo run --release --offline --locked --manifest-path docs/research/experiments/2026-10-26-stokes-kernels/dynamics/Cargo.toml --target-dir /tmp/sunyunbo/blood-stokes-rayon-target --features reciprocal,fft-slices -- --count 64 --omega 0.5 --outdir /tmp/blood-stokes-kernels
```

`run.py --case baseline-r1 --frames 2 --out /tmp/...` 可运行短案例。默认 profiling 关闭；`STOKES_PROFILE=1` 开启既有计时及额外 `profile.csv`，该文件在整步计时结束后写出。主性能与 profiling 分开统计。

CPU 2–5、共用研究文件锁、release、offline、locked。每组主测 360 帧、profiling 120 帧，剔除前 30 帧后使用 nearest rank p95。源码、锁文件、二进制哈希和命令在 [manifest.json](results/manifest.json)，数值对照输出在 [compare.csv](solver/results/compare.csv)。`analyze.py` 核对源码哈希、完整帧数、实际 Rapier 线程数、有限值、交换冲量、方程残差及逐细胞差异；冷启动基线须复现上一轮轨迹。

结果与范围见[报告](../../2026-10-26-stokes-kernels.md)。当前组合流体均值 3.56–3.66 ms、整步 5.92–6.06 ms；不是正式地图或目标设备的性能保证。
