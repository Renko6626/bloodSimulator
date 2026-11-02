# Final whole-branch review

审阅范围：`502ff54..b155e4c`，工作树 `/data/sunyunbo/.codex/worktrees/4aad/bloodSimulator`，HEAD `b155e4ce5032e9ccd2c47a5df46a50cb2f77412d`。采用指定 code-reviewer 模板；本次不派子 agent，仅只读检查，唯一写入为本 scratch 报告。

## Strengths

- 地图落地与已确认候选 A 一致。32 条命名道路以私有固定数据保存，共享坐标真正拆成图路口，普通转角仍留在 centerline；四腔原轮廓只作平移，四个端口局部增加 20 单位窄径接续，保留两条 D3、两片外网及整体跨区尺度。没有新增地图格式、模型、依赖、路由框架或内容系统（`crates/simulation/src/map/organ.rs:13`、`:291`、`:329`、`:337`、`:373`）。
- 预览身份贯穿 runtime、bridge 和 Godot。私有枚举仅 Heart/Organ；旧 `is_heart_preview` 只识别 Heart，共享查询和零配置用于两者；reset 复用对应构造并保留暂停。参数入口及每步仍归零，Godot 互斥和 HUD/F 保护复用同一预览语义，默认入口未替换（`crates/simulation/src/runtime/geometry_preview.rs:4`、`:9`、`:23`、`:34`；`crates/simulation/src/runtime.rs:500`、`:563`；`crates/godot_bridge/src/lib.rs:153`；`godot/world.gd:43`、`:99`、`:230`；`godot/world_hud.gd:139`、`:200`）。
- 验证对应真实风险。地图测试使用实际 MapDefinition/SDF/GraphNetwork，检查共享路口、连通与隔离、真实半径间隔和节点守恒；双模式测试覆盖配置污染、tick、暂停单步和 reset。显式探针预先检查所有 37 个连续段，再用生产 MoveInput 逐帧检查净空、有限位置、零态和 25 块，不把长路线塞入默认测试（`crates/simulation/src/map/organ.rs:413`、`:458`、`:529`；`crates/simulation/src/runtime/geometry_preview.rs:62`；`crates/simulation/examples/organ_preview_probe.rs:147`、`:190`、`:211`、`:219`）。
- 单位图数据直接生成 CSV，解决 Task1 并行文本交错的交付问题；有符号流、低流和肺下口 43.408519% 分流均如实保留，未强制均流或填写最终驱动。文档清楚区分单位图诊断、零 runtime 流、草图与实际图以及局部起点证据（`crates/simulation/examples/organ_preview_probe.rs:81`、`:95`；`docs/research/2026-11-02-playable-map-integration/README.md:16`、`:31`、`:45`）。

## Issues

### Critical (Must Fix)

无。

### Important (Should Fix)

无。

### Minor (Nice to Have)

无新增问题。已记录的实际图标签重叠属于既有 map_render 标注限制，工具文档已说明；Task1 测试诊断交错也已有独立结构化 CSV，不需要本轮追加机制或重跑检查。

## Recommendations

按已批准范围收尾即可。主 agent 将本报告归档到 research，并将 task-reviews 中对 scratch 原件的叙述改为稳定证据链接；这不改变实现或现有验证结论，也不需要重复测试。

## Declined to judge

- 任意路线从正常出生点的可达性与完整全图巡游：方案明确只验正常出生代表路线及三个独立初始化的局部案例；报告没有扩大这些证据。
- 人工走图手感、器官任务价值及跨区旅行的最终游戏性：当前是获批尺度的零流几何预览，未新增内容系统或进行人工交互验收。
- 细胞通过性、实际心腔速度场、物理精度、最终 Q/压差和旧预计算资产适用性：本轮明确只做单位图诊断，runtime 保持零流/零细胞，不烘焙或复用旧 Q48。
- GPU/实卡 FPS、性能细化、云实验与既有非相关物理系统：这些工作继续暂停，当前改动与已有检查未提出需要扩展审阅的具体故障。
- 生成 SVG 的逐字文本及 PNG 像素一致性：它们是实际工厂经既有 map_render 导出的交付物；本次检查其来源、导出选择与转换脚本，沿用主 agent 已静态查看两幅 PNG 的证据，不重复图像验收。简单标签重叠已公开记录。
- 发布、push/merge 的执行结果：本轮没有相关授权；Ready-to-merge 仅表示审阅质量结论。

## Assessment

**Ready to merge? Yes.**

**Reasoning:** 新地图、双预览身份、零状态及有限通行诊断跨模块一致，已确认的技术路线和工作范围均得到保留；未发现需要修复的正确性、集成或维护问题。结论限定于本轮几何/碰撞/区块与单位图诊断，不扩展为运行血流、全图任意路线或游戏性验收。

## Checks / evidence

- 已先读完整 review 包的提交列表和 stat，再分段审阅 source 包及必要的实际文件；跳过生成 SVG 巨大单行和 PNG 二进制。阅读两份 spec、实施计划、全局约束、ledger、三个任务审阅及 Task3 报告；未泛查无关性能系统。
- 具体跨模块核对：原心腔提取与新图平移、固定道路拆边和四端口接续；双模式构造、普通构造的 None、step/reset、桥接参数保护、Godot startup/参数/重置/F 调用；独立局部 spawn 与探针真实 tick；GraphNetwork 单位求解及 CSV/README 的符号和分母。必要时只读查看 ChamberGeometry、既有 GraphNetwork 和 run.sh 以判断平移、诊断和启动命令的实际含义。
- 已读原始 `logs/probe.log`：4 案例共 15387 步，最小实际中心净空 1.415537，全部逐段到达，25 块及零态守卫通过；单位泵 Q=3.393265724、最大节点残差约 1.79e-7。与代码、CSV 和报告一致。
- 已核对 `logs/check.log`、`feature-test.log`、`feature-check.log` 的检查结果及 `results.tsv`：`bash scripts/check.sh` 默认 50 单元+5 集成通过，feature 63 单元+5 集成通过、10 忽略，all-target feature 编译成功。已读两预览加载与双 flag 拒绝原始日志：两加载退出 0，互斥明确报错且预期退出 1。未将忽略的 GPU 诊断计为已运行。
- 本次未运行测试、probe、Godot、基准、烘焙、云实验或自动 UI 操作。只运行只读文件/git 检查；写报告前 `git status --short` 为空且 HEAD 为上述审阅提交。没有修改代码、索引、HEAD 或分支状态。
