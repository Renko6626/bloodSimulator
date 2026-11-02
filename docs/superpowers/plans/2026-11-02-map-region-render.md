# 坐标地图导出工具实施计划

> For agentic workers: superpowers:subagent-driven-development，沿用sol medium实施、sol high审阅。

**Goal:** 提供按任意世界矩形导出当前地图SVG的基础命令。
**Architecture:** 独立Rust example直接读MapDefinition，两个例子共享已有SVG几何片段写入；不进入模拟或游戏接口。
**Tech Stack:** 现有Rust标准库/SVG，无新依赖。
**Spec:** [已确认路线](../specs/2026-11-02-map-region-render-design.md)，用户明确要求“单纯坐标，做一下功能”，按该路线实施，不重复确认。

## Global Constraints

- `--rect X Y WIDTH HEIGHT`及`--output FILE.svg`必填；world units，Y向下，负坐标支持，宽高正且有限。
- `--map heart|small-world`默认heart；可选`--grid STEP`世界原点对齐、`--labels`可见管段编号/内径与心腔名称；无区域名预设或GUI。
- 真实边界/管径、完整交叉视口血管、保持比例，SVG文字转义；默认画布长边约1200像素。
- 仅MapDefinition，不创建Simulation/GraphNetwork/Rapier/Godot/LBM；不输出伪流向或流速。
- 仅example层共享已有绘图片段，无新crate/依赖、生产接口、配置格式、通用编辑/渲染框架或原生PNG后端。
- 旧heart_preview_probe CSV/三个区域/Q提示/域标记/移动及守卫保留；禁止长回放/引擎/GPU/云/性能实验，不自动push/merge。
- 少量关键行为测试集中在新例子，不因抽共享模块重复测试，不跑无关全量套件。

## Review Focus

- 负数CLI参数不能被误当选项；缺失、非有限/非正尺寸应清晰报错。
- 区域中段经过、端点均在外的血管仍须出现；labels锚点应在可见部分。
- 网格按世界原点对齐，不能从裁切左上角重新排网格；过密网格简单报错，不无界写入。
- 大小跨度不同的选区都保持比例，真实宽度不随边界裁剪被缩成同粗线。
- 新命令不误入20分钟回放；旧诊断导出兼容且不改任何模拟源文件。

### Task 1: 坐标导出、共享几何及使用记录

**Ownership:** 新建`crates/simulation/examples/map_render.rs`、`examples/support/map_svg.rs`；修改`examples/heart_preview_probe.rs`仅抽取既有SVG几何写入；文档`docs/tools/map-render.md`、`docs/STATUS.md`及`docs/research/2026-11-02-map-region-render/`。无跨任务接口依赖；共享函数签名由本任务实施者按最小复用确定，不扩大为生产API。

- [x] 实现纯地图构造的CLI、坐标裁切、可选网格/标注和SVG输出。复用真实MapDefinition字段；常规输入检查/文件错误用简明返回，不添加恢复状态机或庞大CLI库。
- [x] 抽共享的几何写入片段并替换旧探针重复代码，保留其数据与行为；新命令不依赖流量求解。
- [x] 在map_render例子中补少量真实测试，覆盖上述关键行为；SVG输出字符串是可观察产物，可断言其属性/内容，禁止源码字符串检查或复制SDF算法。
- [x] 运行`cargo test --locked -p simulation --example map_render`、`cargo fmt --all --check`、`cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu`；旧探针只跑diagnostics-only，新命令实际生成心脏/微管/小地图区域，XML解析并静态查看PNG。全体检查集中一次，局部修正只补验相关项；不跑全量模拟测试和长回放。
- [x] 保存示例图和简短命令/结果/源码来源，更新使用说明及STATUS；自审本地提交拥有文件，不force-add SDD scratch。Task审阅及最终整分支审阅由主agent安排，完成后保留工作树。

最终结果：例子级3项测试、格式、workspace all-target feature编译、新旧诊断导出、XML解析及静态PNG查看通过。逐任务与最终整分支审阅Spec/Quality PASS，无待修复发现；[记录](../../research/2026-11-02-map-region-render/README.md)已归档。未进行模拟/引擎/长回放验证，也不改变游戏行为。
