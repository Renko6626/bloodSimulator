# 细菌美术实施记录

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task after design confirmation. Steps use checkbox (`- [x]`) syntax for tracking. No delegation is requested for this small local change.

**Goal:** 将两种细菌的显微材质、荚膜和游动细菌装饰鞭毛接入战斗预览。

**Architecture:** 复用现有Rust软体实体与Godot World网格。装饰只读取已有轮廓、实体ID、模拟时间及死亡状态。短杆参考形状按后续接入指令实施为局部模拟改动。

**Tech Stack:** 现有Rust／Rapier、Godot GDScript／ArrayMesh、PNG，不新增依赖。

**Spec:** [设计草案](../specs/2026-11-05-bacteria-art-design.md)。用户确认后已执行；结果与实际截图见 `docs/art/bacteria/README.md`。

## Global Constraints

- 继续软体；鞭毛不参与碰撞、推进或攻击。
- 机器人、红／白细胞、地图、流体与战斗规则不在此次改动范围。
- 默认只进行静态截图检查，不自动点击或按键；不启动性能、长回放、烘焙或云实验。
- 不为纹理和样式增加永久测试；实际参考轮廓改动在现有cell测试层覆盖。
- 不因准备素材自动提交、推送或合并；未经确认不实施新的模型／接口决策。

## Review Focus

- 鞭毛根部不能在形变时脱离膜：任务2截图检查。
- 暂停时尾巴不继续摆动，死亡尾巴按设计消失：任务2有限样板检查。
- 剔除边缘仍可见的装饰不能突然消失：任务2边缘静态构图检查。
- 尸体材质与荚膜一同淡出，红／白细胞不受纹理分支污染：任务2截图与既有网格检查。
- 短杆长轴不能绕过墙边放置／接触前筛选：任务1定点审阅和受影响行为检查。

### Task 1: 确认并接入短杆软体参考轮廓

**Files:** `crates/simulation/src/cell.rs`；按调查结果定点检查 `runtime/immunity.rs` 的放置和接触筛选。

**Interfaces:** 复用 `species_builder(center, species, id, config) -> SoftBodyBuilder`；不新增snapshot字段。

- [x] 用户确认方案可用并要求实装；已采用约2.6:1、保持原面积的短杆参考轮廓。
- [x] 在已有cell测试模块补一个关键行为用例，使用真实 `insert_species`／休眠恢复，检查初始非圆轮廓、面积保持及恢复后参考形状一致；复用既有辅助函数，不复制生产形状公式。
- [x] 仅对 `MotileBacterium` 生成圆端杆形并归一化到原面积；荚膜细菌仍为原圆盘。材料节点顺序、节点数、质量、材料不改。
- [x] 核对放置和接触筛选的半径语义。需要超出局部调整时先报告决策点。
- [x] 运行 `cargo test --locked -p simulation cell::tests`；完成所有Rust修改后运行 `cargo test --locked -p simulation`、`cargo fmt --all --check`、`bash scripts/build.sh`。若有失败，仅追加受影响检查。

### Task 2: 菌体纹理、荚膜和装饰鞭毛

**Files:** `godot/world.gd`、`godot/assets/cells/motile-cytoplasm-v1.png`、`godot/assets/cells/encapsulated-cytoplasm-v1.png`、`godot/assets/cells/SOURCES.md`。

**Interfaces:** 读取现有 `state.time`、`cell_status.id/species/dead/opacity`、`state.cells` 及 `CellMesh.closed_contour`；复用 `CellMesh.update`。

- [x] 从 `docs/art/bacteria/candidates/` 复制两张已查看候选到生产素材目录，记录来源、提示词及候选状态。
- [x] 两种细菌进入已有纹理网格路径，去掉旧双层实色深色圆心；轮廓仍随真实软体，死亡染色和淡出复用现有逻辑。
- [x] 用薄、低对比渐淡轮廓叠层替换荚膜硬线圈，身体与荚膜同步形变／淡出。
- [x] 在World添加局部鞭毛绘制方法，从固定材料端定位根部，用模拟时间加ID相位计算两条装饰曲线；活体绘制，尸体不绘制。不添加物理、目标逻辑或跨帧动画状态。
- [x] 将细菌可见范围扩到覆盖鞭毛和荚膜，不扩大模拟窗口。
- [x] 运行 `godot --headless --path godot --editor --import --quit` 和 `godot --headless --path godot --fixed-fps 60 --quit-after 180 -- --combat-preview`，核对日志无脚本错误，不只看退出码。
- [x] 在可用X显示下运行已有 `godot/tests/world_cell_mesh_test.gd`；只在改动涉及网格公共逻辑时增加该部分验证。
- [x] 参考既有美术截图脚本制作本轮有限截图：正常缩放、放大、两个模拟时刻，以及暂停／死亡／视口边缘的静态样板。查看实际图像，不自动操作UI，不扩展成新交互测试框架。
- [x] 根据实际小尺寸截图最多做必要的材质、荚膜厚度和鞭毛幅度调整；新改动仅复查受影响项。

### Task 3: 记录交付边界

**Files:** `docs/art/bacteria/README.md`、`SHA256SUMS`，接入完成后按实情更新 `docs/STATUS.md`。

- [x] 保存实际截图及运行命令／结果，将生成草图、材质候选和游戏实装证据分开注明。
- [x] 运行素材哈希检查和 `git diff --check`；核对无无关产品文件修改。
- [x] 报告实际通过项及未验证项，不声称已验证FPS、完整战斗手感或极端形变。


## 实施选择与证据

- 本轮自行完成实现；按executing-plans技能安排一次独立只读代码审阅，不委派实现任务。
- 沿用现有World入口和CellMesh，不新增通用模块、动画状态机或bridge字段。
- 常规样式检查使用实际截图，没有为美术样式新增永久测试；仅短杆物理参考形状新增一个真实行为用例。
- 逐段鞭毛绘制在放大图出现深色接点，改为单条连续抗锯齿折线；保留每条16段的行波点。此为局部绘制细化，不是性能优化项目。
- 冻结场景截图脚本切换相机后显式刷新相机变换；死亡／形变是显示快照样板，未伪称实际游戏流程测试。
- `cargo test --locked -p simulation`：60＋5通过；桥构建、fmt、diff、引擎导入、战斗180帧、已有GL网格检查及静态截图通过。参考形状用例的RED→GREEN证据在本轮工具输出。
- 实现已提交为 `d2a756f` 并快进纳入main，本轮推送；未进行FPS、人工手感、极端形变验收。

2026-11-05 形状细化：用户要求更长、更像杆菌；当前比例2.6∶1，端部单独分配节点保留圆钝外形。4项cell与12项immunity受影响检查、桥构建、180帧加载及截图通过；未重复全套测试。

2026-11-05 荚膜细化：按用户反馈改为厚薄略不均的三层胶质外套，现有纹理淡叠在外套上。仅改World绘制，导入、180帧加载、正常／放大／变形／死亡显示截图及diff通过；局部只读复查通过，无Rust改动或新测试。

2026-11-05 主目录实装：按用户指令将完整细菌美术同步到主目录main工作区，产品源码和两张纹理与已审阅工作树逐字节一致。只同步工作区，不创建提交、合并提交或推送；功能工作树保留。

最终集成：用户随后明确要求提交推送并合并main，`d2a756f` 已从功能分支快进纳入主目录。合并前50个主目录修改／未跟踪文件逐字节核对并在合并后复核；无冲突或额外产品改动。主目录桥构建、导入、10帧启动、fmt／diff和素材校验通过，沿用已完成模拟与截图证据，不重复整套验证。
