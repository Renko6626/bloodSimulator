# RTX 3060：GPU Stokes 分段计时

用户要求在 Vast.ai 上进一步定位 GPU 耗时。沿用 RTX 3060／Xeon 实例、冻结 Linux 输入和原算法；本轮仅新增可选诊断字段与时间戳，不改变 shader、精度或 CPU LU 默认后端。

## 运行方式与计时口径

```bash
cargo build --release --locked -p fluid-bench --features gpu
target/release/fluid-bench run --backend gpu-fgmres-mg \
  --case DATA/wide192/region-0 --limit 4 --mg-cycles 1 \
  --profile host --out NEW-host.json
# 同样参数分别改为 --profile off、--profile gpu，使用新的输出文件
```

统一脚本 `scripts/fluid-experiment.py` 也接受 `--profile off|host|gpu`，只对 GPU MG 后端启用诊断。off 为默认；host 记录 wall timer；gpu 额外请求可选 TIMESTAMP_QUERY，能力或单批查询容量不足时明确报告 `gpu_timestamps_enabled:false`，仍保留主机计时。此特性不成为玩家设备要求，也未引入新依赖。

- `host_ms` 为互不重复的阶段 wall time；`unaccounted_ms` 为总耗时减去这些阶段。它们包含所在 API 内的阻塞，不等同于纯 CPU 运算时间。
- `host_detail_ms` 是编码、回读准备的子项，已包含在父项中，不能再相加到总耗时。
- `gpu_stages` 使用每个 pass 的起止查询，按 `queue.get_timestamp_period()` 换算 ms，并记录工作组数。它们不是纯浮点计算时间，也包含该 pass 的执行与计时干预。
- `gpu_interval_span_ms` 是每次读状态前，第一项到最后一项查询的跨度，包含 pass 间空隙和设备闲置。CPU 和 GPU 时间轴会重叠，不能把 host 与 GPU 数字相加。
- 额外查询映射在 `profile_collect_ms`、`profile_readbacks` 中单列；算法本身的 dispatch、提交和读状态计数保留。提前收敛后批次内的 gated dispatch 也计入。

查询槽覆盖已有八步批次；连续 finish／下一次 start 使用不同槽位及回读偏移。resolve 从对齐偏移 0 读取各自范围，避免后一次写入覆盖尚未统计的查询。

## 第一轮：定位大阶段

三个规模的 region-0、前四个真实 RHS、1／2 次 V-cycle、off／host／gpu 三模式，两轮反序交错，共 144 次求解全部通过原方程 1e-4 检查。48 组匹配输入在三模式下的迭代数、停止残差和原方程／参考解诊断逐项相同。所有 timestamp 模式的阶段 dispatch 数之和与求解器计数相符。

每格为四个 RHS、两轮共八条记录的均值。以下是 **host 模式**的 wall time，单位 ms：

| 场景／V-cycle | 求解总计 | 命令编码 | 提交 | 回读准备 | 映射／poll | 原方程审核 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| small64／1 | 67.36 | 36.49 | 2.40 | 17.90 | 4.59 | 0.22 |
| small64／2 | 88.37 | 51.99 | 2.00 | 16.35 | 11.53 | 0.22 |
| wide96／1 | 121.35 | 60.75 | 3.67 | 32.19 | 13.85 | 0.28 |
| wide96／2 | 168.78 | 95.10 | 4.10 | 26.90 | 32.92 | 0.30 |
| wide192／1 | 193.69 | 88.10 | 8.77 | 49.26 | 33.14 | 1.61 |
| wide192／2 | 227.08 | 112.49 | 6.04 | 31.11 | 65.09 | 1.56 |

命令编码约占 45%–59%，是本轮最大的单项；上传和解映射很小，审核不是主要成本。回读准备也可能包含驱动阻塞，不能仅从这张表把它认定为内存分配计算。

下表来自另一组 **GPU timestamp 模式**，不能和上表拼成一个总耗时：

| 场景／V-cycle | GPU pass 时长之和 ms | MG 平滑 ms | GPU 查询跨度 ms |
| --- | ---: | ---: | ---: |
| small64／1 | 9.53 | 4.65 | 17.39 |
| small64／2 | 12.93 | 7.30 | 21.88 |
| wide96／1 | 23.30 | 10.99 | 40.36 |
| wide96／2 | 32.54 | 17.33 | 56.12 |
| wide192／1 | 43.98 | 21.83 | 65.60 |
| wide192／2 | 60.25 | 33.17 | 90.27 |

设备侧最大类别是 MG Jacobi 平滑，其后主要为 MG 传递／残差和归约。约 23%–29% 的 dispatch 仅有一个工作组，大量粗层小任务不能充分展开整卡并行；这也不是大规模密集矩阵乘法的吞吐场景。

两次 V-cycle 配置中，完整 Stokes 算子的 SpMV pass 在三个规模合计分别约 0.108／0.281／0.500 ms；这不包含 MG 内部残差／传递／平滑。GPU 对这一算子的执行确实很快，不能把整个求解入口的百毫秒都解释为矩阵算术慢。

## 第二轮：定位到具体 API

第一轮显示编码与回读准备占比高，随后只增加嵌套 wall timer，测三规模初始 RHS、1／2 次循环、两轮；额外加入旧二进制对照，共 48 次求解全部通过。12 组输入在 original／off／host／gpu 四种情况下的迭代、停止残差、原方程与解差诊断相同。

以下为 host 模式两个初始样本记录的均值，单位 ms；与第一轮四 RHS 表格口径不同：

| 场景／V-cycle | 编码总计 | encoder.finish | 回读准备总计 | create_buffer |
| --- | ---: | ---: | ---: | ---: |
| small64／1 | 41.90 | 40.54 | 17.52 | 17.25 |
| small64／2 | 75.53 | 73.81 | 19.07 | 18.86 |
| wide96／1 | 79.50 | 76.48 | 36.53 | 36.21 |
| wide96／2 | 114.00 | 109.55 | 31.50 | 31.23 |
| wide192／1 | 93.53 | 88.76 | 40.39 | 40.04 |
| wide192／2 | 145.16 | 139.25 | 36.28 | 35.87 |

编码成本约 95%–98% 集中在 `encoder.finish()` API。begin pass、设置 pipeline／bind group、dispatch 和 end pass 的 Rust API 调用本身合计较小；因此不能直接把全部编码耗时归给外层 for 循环或 pass 析构。

回读准备中约 98%–99% 集中在 `device.create_buffer()`。当前每次读状态／解都新建 staging buffer，这是可直接针对的行为；wall timer 不能区分底层驱动分配、资源锁和等待。不能据此宣称复用缓冲必然省下全部这段时间，须用后续单项对照确认。

旧二进制与新版 off 的同机初始输入均值差约为 −3.5% 至 +13.2%，每格只有两次记录，CPU／驱动环境存在波动，不能证明诊断代码零开销，也不足以将差异归为一次稳定的性能回归。所有数字仍是定位证据，本轮没有优化提速声明。

## profiling 对运行的干预

| 场景／V-cycle | off 总耗时 ms | host 总耗时 ms | gpu 总耗时 ms |
| --- | ---: | ---: | ---: |
| small64／1 | 73.58 | 67.36 | 64.47 |
| small64／2 | 94.81 | 88.37 | 89.80 |
| wide96／1 | 124.14 | 121.35 | 125.38 |
| wide96／2 | 162.67 | 168.78 | 160.77 |
| wide192／1 | 166.37 | 193.69 | 153.87 |
| wide192／2 | 248.95 | 227.08 | 230.44 |

未固定宿主 CPU 负载和频率，off／host／gpu 只做顺序交错，单项存在变化；时间戳模式还会移动部分 API 阻塞的位置。不能将某个仪器化模式更快视为算法优化，也不能据此得出一个固定的“profiling 开销百分比”。本轮无新增 cgroup CPU 配额节流。

## CFD GPU 加速与下一步

NVIDIA 的 [AmgX](https://github.com/NVIDIA/AMGX)提供成熟的 GPU 多重网格和预条件 Krylov 实现，覆盖单卡／多卡、fp32／fp64 等组合。GPU 稀疏求解确实有可参考的工程实现。[NVIDIA 的说明](https://docs.nvidia.com/dl-cuda-graph/latest/cuda-graph-basics/quantitative-benefits.html)也明确指出大量小 kernel 可能受启动与主机供给限制；这里引用其机制，不移用文档中的加速倍数。

对当前程序，数据支持先以单项实验验证复用回读 staging buffer，再减少 `encoder.finish()` 需要处理的小 pass／dispatch 数，并优化 MG 平滑与层间工作量。尚未区分 finish 内 wgpu 与驱动各占多少。当前完整 Stokes SpMV 本身远小于编码成本；MG 还包含大量其他稀疏与向量操作，不能用单个 SpMV 的速度代表整个求解器。CPU LU 的固定因子回代与 GPU 每次几十轮迭代也承担不同工作量。

这一轮没有融合 kernel、改变 pass 边界、放宽精度或采用 CUDA。后续合并／融合须验证跨 dispatch 数据依赖与原方程收敛，不能直接删掉同步边界。

## 验证与原始证据

- 第一轮[完整 manifest](validation/vast-profile-v1/manifest.json)、[指标](validation/vast-profile-v1/metrics.json)、[实际运行脚本](validation/vast-profile-v1/run.py)。该轮的源码快照保留为 `source.tar.gz`，解压后的 SHA256 与 build.json 一致。
- 细分轮[完整 manifest](validation/vast-profile-detail/manifest.json)、[指标](validation/vast-profile-detail/metrics.json)、[实际运行脚本](validation/vast-profile-detail/run.py)。细分字段是父项的子计时，不重复加入总计；检查子项之和不超父项，并核对查询 dispatch 数。
- `bash scripts/check.sh` 通过，涵盖现有 simulation 测试、扩展构建和两个 Godot headless 场景。
- `cargo test --offline --locked -p blood-fluid -p blood-fluid-gpu` 通过。新增测试核对 timestamp 周期换算、分类累计以及 pass 间空隙；软件 GPU 的真实 64² 样本通过，查询计数覆盖全部 1,564 个 dispatch。
- 两次只读审阅未发现重要问题。物理设备检查保留真实原方程审核，没有用计时数字代替正确性验收。
- 新的 profiling 功能本轮实测覆盖 Linux RTX 3060 和显式 llvmpipe；Windows 时间戳诊断未实机复测。
