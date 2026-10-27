# Vast.ai RTX 3060 原始记录

- `manifest.json`、逐次 JSON 和 `run.py` 是本轮实际执行的原始快照；`build.json` 记录本地构建及可执行文件、输入哈希。没有上传私钥或账户令牌。
- **输入标注更正：** 原始 manifest 的 `options.input` 中“与 Windows 数据逐字节一致”是采集来源的错误假设。`input-audit.json` 通过实际 SHA256 对照确认：18 个矩阵二进制和 12 个初始 RHS／LU 向量全部一致；后续 78 个动态向量有 6 个一致、72 个不同。原记录保留供审计，以此更正和实际哈希为准。不能把全部样本的跨机耗时比视为严格控制输入的加速比。
- 本轮内部所有后端和 1／2 次配置都读取同一份冻结 Linux 数据，因此同机的比较与收敛检查有效。
- `metrics.json` 汇总每规模／后端／循环数的两域两轮 16 条记录。`mean_non_readback_wait_ms` 是总耗时减去回读函数计时，包含编码、提交、上传和原方程审核等尚未细分的工作，不能当成严格隔离的 CPU 或 GPU 时间。
- 复现目录布局是 `bin/fluid-bench`、`build.json`、`source.tar`、`source/`、`data/`、`run.py`。源码为 f3aaa4f；先按 build.json 的命令构建 Linux x64 程序，采集或放入经哈希核对的数据，复制到 bin 后执行 `python3 run.py`。每次使用新目录，results 必须不存在。若重新构建或重新采集，应生成属于该次运行的新 build.json；不要沿用本轮二进制或时序文件的旧哈希。
