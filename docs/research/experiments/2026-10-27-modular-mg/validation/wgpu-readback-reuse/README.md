# wgpu 回读缓冲复用原始记录

本轮只改变 MgSolver 的状态／解回读缓冲生命周期：两个缓冲在 prepare 中创建，读取后释放 view、unmap，再复用。拷贝、提交、shader、迭代和容差保持。

baseline 为此前 profiling 版，精确二进制哈希及其源码提交在 build.json；reuse 源码快照为 source.tar.gz，解压后的 SHA256 对应 build.json.source_archive_sha256。base_commit 为 7b604e1，源代码修改状态明确记录为 true。最终源码文件哈希可直接核对。

run.py 是实际云端脚本：三规模／两区域／前四 RHS／1、2 次循环／baseline、reuse／off、host／两轮交错，共 384 次求解。每个配置有 16 条计时记录。manifest 保留命令和原始结果；metrics 为分组均值／中位数等，comparison 为正常模式匹配对照。host_detail 子计时属于 host_ms 父项，不能重复相加。

所有后端使用同一冻结 Linux 数据；动态 RHS 不保证与 Windows 字节相同。192 对前后版本的迭代、停止残差、原方程与参考解诊断、dispatch／提交／回读计数均逐项一致。

运行布局为 bin/fluid-bench、source.tar、source/、data/、build.json 和 run.py，baseline 来自邻目录 blood-profile-detail-f3aaa4f/bin/fluid-bench，并在执行前核对哈希。复现使用新目录和当前构建／输入的新哈希，不能沿用旧二进制哈希。实验实例已停止，回执只含状态和实例 ID 等非凭据字段。
