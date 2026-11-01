# 心脏分级血管布局记录

2026-11-01，已确认的完整分级方案在 `feat/heart-vessel-layout` 实施。地图沿用现有SDF、GraphNetwork、固定16单位区块和 `--heart-preview`，不增加运行血流或细胞。下列诊断图直接读取生产 `MapDefinition::heart_preview()`；不是游戏截图，也不是流场图。

## 地图与诊断图

实际地图98节点、224边；肺、体各106条分级边，各64条独立D3微管。每套动静脉合计D24两条、D12八条、D6三十二条。另12边保留心脏内部及D9/10连接。包含管壁半径的全图范围为 `[-712,-1305.5]..[712,1413.5]`。

- [全图SVG](overview.svg) / [PNG](overview.png)：上方肺模块、下方体模块，经心脏串联。橙D24、绿D12、蓝D6、浅紫D3，心腔及窄口为红色。所有实线宽度来自真实管径。
- [心脏端口SVG](heart-ports.svg) / [PNG](heart-ports.png)：端口轴线x38/78/112/150、y=-32，D10/D9/D10/D10。白色虚线仅标记候选心脏烘焙域 `[-32,224]²`，不表示管道或流场。
- [肺微管细节SVG](lung-micro-detail.svg) / [PNG](lung-micro-detail.png)：展示部分真实D6分叉、D3并联通路及用于实际等长的绕行；四条微管仅共享合法两端分叉，途中有组织分隔。

四腔曲线、左右隔墙、房室口、出生点保持。D24端帽在心脏域外；体循环通过x±700外廊连接下方模块。真实SDF的隔墙/房室口、全部128微管中心和126中缝组织分隔、外管非相邻几何分离均由Task1生产逻辑检查。Task1报告的最小5单位间隔指不同外边的非分叉胶囊，不包括同边发夹内部间隔。

## 实际图求解

[actual-graph-flow.csv](actual-graph-flow.csv)为生产 `GraphNetwork::new(&map, Config::default().viscosity)` 与两次 `solve()` 的逐边输出，共224条，不是先前[理论计算表](flow-table.csv)。默认粘度0.05时，单位驱动流量1.5853244，归一化到循环Q48的参考驱动为 **30.277714**。这是现有阻力图模型的等效驱动，不是生理心腔压力，未接入预览。Task1在粘度1下的605.5543是另一个明确标注的参考值，不能代替默认粘度结果。

| 管径 | 两套合计边数 | 单边Q | H=1截面均速Q/D | CSV最大绝对Q误差 |
| --- | ---: | ---: | ---: | ---: |
| D24 |4|48|2|0|
| D12 |16|12|1|0|
| D6 |64|3|0.5|0|
| D3 |128|0.75|0.25|6e-8|

逐边rate保留有符号图方向；表中不将串联的两套Q48相加。均速由 `rates[i]/diameter` 得到，不使用旧圆截面积 `summary.mean_speed`。CSV输出精度下，98节点流入流出残差最大为0；Task1直接f32检查残差低于1e-4。窄口同样承载Q48。

## 实际输入回放

修正后的完整有界回放退出0，共 **211个导航点、87771tick**，按每帧机器人位置累计约11684.384单位。右/左心室实际跨度 **83.985/89.943**；全程最小实际中心净空 **1.273**，位置约 `[-15.7392845,-1303.7727]`（肺微管段）；现场块峰值 **25**。每帧零红白细胞、零分支流量、零总流量/压差守卫通过，最终净空和峰值断言通过。覆盖右室深部→肺完整微管支路→左室深部→体完整微管支路→右房返回。

[实际回放日志](actual-probe.log)保存每个到达坐标、SDF中心净空及加载块数。生产tick和MoveInput推进，速度8、原时间步/子步、机器人半径0.85、每点2400tick限制及1.0中心净空阈值保持，无瞬移。

初次210点回放已到达全部点，但最终净空守卫失败：[首次失败日志](actual-probe-first-failure.log)记录87761ticks、约11683.146单位；最终返回右房的直接连线遗漏真实入口折点(38,32)，在 `[42.148014,24.957287]` 中心净空降到0.85198593。首轮每个到达点记录25块，但最终峰值断言在失败后未执行，不用它代替峰值验收。Task1在 `5ea9431` 补回折点并增加生产SDF连续路线检查；独立复审通过。探针新增相同1.0阈值的逐帧及时失败断言并保留最终守卫；随后才进行上述修正后完整回放。两次实际源码快照和来源均保存，没有降低守卫或更改几何来掩盖失败。

## 集中检查与来源

全部命令复用 `CARGO_TARGET_DIR=/data/sunyunbo/www/bloodSimulator/target`。`bash scripts/check.sh` 退出0，含fmt/workspace检查、47单元+5集成、bridge构建、Godot editor导入及普通world/lab加载；首次导入退出0。`cargo test --locked -p simulation --features lbm-gpu` 退出0，60单元+5集成、10忽略；`cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu` 最终退出0。headless `--heart-preview` 180帧加载退出0。

图标题布局调整曾触发E0689浮点类型不明确，保留[首次编译失败](feature-all-targets-first-failure.log)；补明确f32后只重跑fmt、受影响全目标编译和诊断导出。标题修正没有触发完整Rust套件或实际长回放重跑；移动/物理未改。真实路线失败修正后，只补跑fmt、全目标feature编译、bridge构建、heart-preview加载及完整实际回放，均退出0。默认/feature套件不重复；对应[fmt](fmt-corrected-route.log)、[编译](feature-all-targets-corrected-route.log)、[构建](build-corrected-route.log)、[加载](heart-load-corrected-route.log)日志保留。[命令退出记录](command-exits.log)、[默认检查](check.log)、[feature测试](feature-tests.log)、[全目标编译](feature-all-targets.log)、[heart-preview加载](heart-load.log)保留实际输出。

诊断源和重现命令见[来源记录](provenance.md)。未改GDScript/HUD，Godot仅做headless加载，无自动按键或点击。只验收几何、生产图参考流量及本条零状态通路；不代表人工手感、任意路径、心腔场/物理精度、细胞负载、动态瓣膜/收缩、烘焙或实卡性能验收。没有启动云实例或性能实验。最终整分支独立审阅由主agent安排。
