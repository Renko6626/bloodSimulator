# GPU profiling 原始记录

本目录保留实际 run.py、build.json、逐次 JSON、manifest 和聚合 metrics。source.tar.gz 解压后的 SHA256 对应 build.json 的 source_archive_sha256；它是当时的未提交源码快照，不包含凭据。base_commit 为 f3aaa4f，源码修改状态明确标为 true，实际文件哈希是精确来源。

同一冻结 Linux 数据用于所有模式，动态向量不保证与 Windows 逐字节相同。所有三模式（细分轮还含旧二进制）都通过原方程检查；对照时不要把不同模式的 host 与 GPU 数值相加。host_detail_ms 已包含在 host_ms 父项中。profiling 自身会改变运行，normal/off 的端到端时间才是非仪器化基线。

第一轮为三规模 region-0、四个 RHS、两轮、1／2 次循环和 off／host／gpu，144 条记录。细分轮为同区域初始 RHS、两轮、1／2 次循环和 original／off／host／gpu，48 条记录，用于定位 API 内部并检查旧程序对照。两轮统计口径不同，不能混为同一组平均。

目录布局与 baseline 类似：bin/fluid-bench、source.tar、source/、data/、build.json、run.py。细分轮还使用邻目录 blood-experiment-f3aaa4f/bin/fluid-bench 作为 original，哈希已记录并在执行前核对。复现时使用新目录；自行构建的二进制或重新生成的数据须生成新的 build.json，不沿用历史哈希。原云端目录与 bin 保留，实验进程已退出。
