# 离线 LBM 背景资产基础设施

本阶段在 `feat/offline-lbm-bake` 共用现有 D2Q9 MRT、Guo 外力、LSQ-FD 和帧末均值控制，补齐固定域求解→时间平均→资产读写→正常背景导入的管线。没有复制 shader、新建 crate 或引入依赖。默认地图、实时16单位块和48×48移动窗口不变。

## 实现

- `DomainSpec` 与 `LbmDomain` 支持显式正方形固定域；`LbmWindow` 保留原公开接口和移窗迁移。新尺寸、格距、有限范围及设备分配上限不满足时返回错误。
- `bake_mean` 显式推进预热/采样帧，每帧末控制平均密度；按间隔读取实际求解结果并用 f64 累加。报告样本数、实际模拟时长、采样密度、开口通量及前后半均值差。没有自适应停止或收敛判据。
- `to_precomputed_field` 原点严格为 `domain.origin + spacing/2`，速度行序、掩码、求解黏度不变。`resample_precomputed_field` 复用原生产双线性采样，在源支持范围内重采样；新掩码以地图几何为准，壁面/固体置零。参考条件和端口元数据保留，正常背景语义验证仍须执行。
- `BFLOW001` 保存小端元数据、端口、速度和掩码；读写检查结构、有限值和精确负载。它不会替代地图/参考条件/端口校验。
- `offline_lbm_bake` 仅为显式例子；必须给 `--output`，软件适配器只允许成对 `--allow-software --functional`。导入失败返回非零，并在文本报告中保留失败结果。

## 固定夹具和预算

闭环图节点为 `[-48,24]`、`[96,24]`、`[96,96]`、`[-48,96]`，所有边半宽3。域下界 `[-16,-16]`、边长80、h=.5、160²格，域内只有 y=24 的长直管，图节点和回程均在域外。仅 `(0,1)`、`(1,1)` 两块选择预计算，端口 x=0/32，直管缓冲充足。

GraphNetwork 以实际黏度 .05 解出参考压差 `5.432489`，使每边流量为8；二维单位深度为1。原边界开口目标为 -8/+8，dt=.01、每帧4步，解析峰值2对应格子速度 .04。没有用零/微小流量躲避检查。

## 两次实际软件作业

两次均为本机 llvmpipe（LLVM15.0.7，256bits），显式软件功能验证；没有硬件/FPS含义。

首次预算20帧预热＋20帧采样、stride2，共160格子步，.8＋.8模拟秒、10个样本。求解、写入、读回完成，但原格心资产正常导入失败，返回 `static port velocity profile does not match planar analytic flow`。失败输出保存在 [software-functional](software-functional/report.txt)。

按系统排错流程读取实际资产：壁面 y=21/27 位于固体格心20.75/27.25和首排流体格心21.25/26.75之间。x=0壁面双线性速度 .152225、x=32为 .148621，而原容许向量误差仅 .0557566；这属于壁面采样错位引入的一阶误差，直接格心转换本身没有偏移错误。短预算还留下瞬态差异，x=32中心1.955302、壁面对齐后通量约7.702880，单纯改导出网格仍不够。未更改原校验，详见[只读诊断](software-functional/port-diagnosis.txt)。首次预算短于80单位域约2.77秒的声学传播时间，不能据此要求端口已达到稳态。

控制器批准同格距导出重采样和一次新的有界预算：300帧预热＋600帧采样、stride10，共3600格子步，12＋24模拟秒、60样本。该预算没有自适应延长，不声明收敛。保留 [原始格心资产](software-revised/raw-cell-centered.bfld)，由生产双线性采样导出原点 `[-15.5,-15.5]`、159²格、h=.5的[壁面对齐资产](software-revised/straight-pipe.bfld)。没有解析替值、速度归一化或端口容差修改。

修订作业 [report.txt](software-revised/report.txt) 记录：

| 项目 | 实际值 |
| --- | --- |
| 采样密度范围 | .99796957～1.0018934 |
| 平均开口通量 | -7.999999746 / +7.999999746 |
| 前后半均值速度差 RMS / 最大值 | .0004220915 / .0010745374 |
| 最终质量 | 1920.0001 |
| 导出写入/读回速度和掩码 | 精确一致 |
| 原 `try_new_policy_world` 导入 | `Ok(())` |
| 两处交接附近中心背景速度 | 解析2；预计算约1.98273～1.98483 |
| 壁面背景速度、局部覆盖权重 | 均为0 |

`wall_seconds` 仅记录本次作业实际用时（首次约 .673秒、修订约7.236秒），不是性能研究或设备比较。前后半均值差是描述量，不代表统计收敛。

## 命令与检查

从仓库根目录执行，构建缓存设置 `CARGO_TARGET_DIR=/data/sunyunbo/www/bloodSimulator/target`；两次GPU作业另设置 `VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json`。

```sh
cargo run --locked -p simulation --features lbm-gpu --example offline_lbm_bake -- --output docs/research/2026-10-31-offline-lbm-bake/software-revised --allow-software --functional
cargo fmt --all --check
cargo test --locked -p simulation
cargo test --locked -p simulation --features lbm-gpu
cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu
cargo test --locked -p blood-fluid-gpu lbm:: --lib
```

首次命令相同但输出目录为 `software-functional`，当时例子使用初始预算与原始格心导出，退出1；修订命令退出0。原始作业由当时未提交例子运行，最终源码包含上文记录的修订方案；不把最终例子哈希当成首次源文件的哈希。

集中检查均退出0：simulation默认40单元＋5集成，feature53单元＋5集成（10项GPU诊断ignored），受影响fluid-gpu LBM 7项通过（6项ignored）；工作区feature全目标编译与fmt通过。集中检查结果和完整输出见 [checks/results.json](checks/results.json)；源码基线、最终运行源码/资产哈希见 [source-manifest.json](source-manifest.json)。Task1已运行的移窗GPU重叠迁移用例通过，本阶段未重复该作业。新增永久测试仅覆盖固定域/平均累加/资产协议/重采样关键行为，GPU诊断仍保留原有 ignored 标记。

## 验收边界

三项任务及[整分支独立审阅](review.md)均通过，无Critical/Important/Minor；实施取舍与代价见[交接记录](controller-notes.md)。分支保留本地，未自动推送或合并。

支持的是固定正方形域和单份时间平均背景资产。驱动按统一压力比例缩放仍是既有玩法近似。壁面对齐导出适用于本直管例子，不能据此声称任意曲壁/开口资产都能通过原端口契约。

未验收心腔几何、器官地图、统计收敛、物理精度、任意斜率/曲开口、绝对压力、长程耦合或实卡性能。本阶段没有Godot/UI操作、云实例、发布、合并或推送，也不恢复已暂停的性能/实时64窗口研究。
