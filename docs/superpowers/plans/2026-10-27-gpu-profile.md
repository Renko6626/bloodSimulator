# GPU Stokes 分段 profiling

用户要求直接编写 profiling 并在已授权的 Vast.ai 实例测量。沿用三个 crate 和现有 wgpu；这是现有实验入口的诊断扩展，不新增依赖、求解路线或游戏机制。

- [x] `gpu-fgmres-mg` 增加 opt-in `--profile off|host|gpu`。off 保留基线；host 拆分 RHS／编码／提交／回读准备／等待／解映射／审核；gpu 在支持时额外启用 pass 时间戳，不支持则明确只报告 host。
- [x] GPU 按 Jacobi、MG 残差／传递／粗层、Stokes SpMV、正交化／归约／控制等汇总时间与工作组数。查询按已有批次读取，处理 finish 与下一次 start 连续提交的情况，不修改 shader／算法／容差。
- [x] 验证 timestamp 周期换算与分组汇总，运行既有检查；软件设备验证 profile 保持收敛，物理设备对照 off／host／gpu 的实际结果与侵入成本。
- [x] 在 RTX 3060 的同一冻结数据上测三个规模的 region-0、1／2 次 V-cycle、两轮反序，记录原方程审核和源码／二进制哈希。
- [x] 汇总最主要的主机与 GPU 成本、局限和下一步证据；本轮不提前融合 kernel 或更换后端。

另一可行办法是仅做 CPU wall timer，成本低但无法解释 GPU 内部；外部 Nsight 更深入但新增环境准备，本轮先采用内置可选时间戳。时间戳改变指令流，必须与无时间戳基线并列，不用仪器化总耗时宣称提速。

前置实验：`docs/research/experiments/2026-10-27-modular-mg/vast-rtx3060.md`。原始计时的回读函数以外仍有 36–97 ms，但尚不能据此断定全部为纯主机计算。


完成记录：第一轮 144 次回放，三个模式的 48 组匹配输入诊断相同；细分轮含旧程序对照的 48 次回放，12 组四模式诊断相同。主机编码主要集中在 encoder.finish，回读准备主要集中在 create_buffer；尚未区分其内部驱动分配／锁／等待。本轮检查通过，原始快照与两轮口径见研究报告。未融合 kernel 或复用 staging（留作后续单项优化）。
