# 范围限定只读审阅

独立 reviewer 检查新实验的 Rust／WGSL 数值流程、缓冲区范围、uniform 写入与提交次序、采集来源和云端编排；未修改文件、安装依赖或运行硬件负载。

未发现阻断或重要正确性问题。具体检查包括两次 CGS 的新系数／累计系数分离、缩放方程还原、原约束残差验收、未收敛退出码和软件设备标签。

两项低风险记录口径已在 README 说明：

- `hardware_tests_run` 表示硬件 probe／smoke 通过后进入回放流程；失败的探测尝试仍在逐条 runs 中。
- `--allow-software` 可用于限定范围的真实矩阵功能诊断，输出始终标记 `hardware_measurement:false`；cloud 编排不传该选项。

README 在该轮代码审阅之后完成，由主 agent 做链接与记录一致性核对。此次审阅不替代物理 GPU、消费级设备或完整游戏验证。
