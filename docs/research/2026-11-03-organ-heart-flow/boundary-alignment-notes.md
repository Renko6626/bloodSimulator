# SDD ledger — plan: docs/superpowers/plans/2026-11-03-heart-boundary-alignment.md

用户确认烘焙边界和实际交接同面、四口使用同一解析剖面，暂不加混合段。沿用feat/organ-heart-flow工作树，起点579e0b4。

- Pre-flight: Task 1提供显式剖面和对齐导出及元数据；Task 2用同一例子运行原校验，只有通过才执行Task 3嵌入，不存在接口冲突。
- 设计builder检查nothing important left。实施坐标y656是已确认路线内的几何选择；未引入新求解器/格式/来源类型。
- Task 1 CPU测试：boundary_export_keeps_actual_interior_and_prescribed_trace先因未实现返回Err失败，完成后offline_lbm三项均通过。测试直接调用真实导出/背景采样，独立期望边界1.5、内部(.75,.125)、壁面0和拒绝未声明支撑流体。
- Task 1软件GPU小用例：bake_applies_supplied_profile_before_advancing先因仍使用默认剖面得到.14651163而失败，接入原setter后通过（完整作业约.15秒）。这是16²/12格子步关键接口功能验证，不是另一轮心腔烘焙或性能实验。
- Task 1真实CPU预检通过：384²，标签[464,656]..[608,848]，四腔完整；四口沿程10/80.93172/10/86.65863，原2r检查通过。两个流体分量28041/28693格，各分量Q收支0，初场格子峰值<.05。
- Ruling: 导出增加一排域外支撑并在真实边界写出已施加的连续解析条件，内部只用真实原场双线性采样；不使用速度混合、内部归一化或修改原容差。离散法向通量归一和连续解析的O(h²)差别按原容差检查。
- 后续单次心腔作业仍为dt=.0025、16步/逻辑帧、1500预热+1500采样/stride25、48000格子步、60样本，墙钟1200秒。失败停止，不自动重跑或转云。

- Task 1: complete，提交08f3946；已有GPU LBM检查7通过/7忽略，fmt通过；CPU预检记录在alignment-preflight。
- Task 2: attempt-03已按本方案单次预算启动，源码及二进制哈希已冻结。运行首行有遗留512²提示文案，真实DomainSpec/求解器日志/源码与manifest均为384²；启动后已只修正提示文案，不将修正文案源码混称为已运行二进制来源。

- Task 2: complete，单次attempt-03求解225.516590秒，48000步/60样本，原读回及try_new_policy_world均通过，命令退出0。没有放宽容差、内侧混合或后续完整烘焙。
- Task 3: complete，入口提交289901f；默认无GPU构造/reset测试RED→GREEN。4个明确局部起点往返共972步/8切换，最小中心净空4.5，零细胞/无实时LBM。
- Ruling: 新policy HUD均速改为既定H1截面均速，旧图保持圆管口径；这修正显示量口径，不改变图阻力或速度场。默认入口/reset测试独立期望Q48/D10=4.8。
- 相关集中检查：默认51单元+5集成、feature65+5（10忽略）、GPU纯逻辑7通过/7忽略、全目标feature编译通过。check.sh首次Godot冷导入134，明确保留失败；只补cached import及各场景加载，实际均0，两互斥项为期望1。
- 图像：本地已有Xvfb与llvmpipe，只静态截图无键鼠输入；因说明过长缩短文案后仅补截图，未重复整套检查。工作树target采用本地缓存软链接，已加入本地info/exclude，不提交链接。
- Final review: 独立整分支审阅无Critical/Important/Minor；核对资产b1760e82与972步。Declined to judge按实际边界保留：细胞交通/人工体验未做，物理精度/均值稳定/FPS不由此证明，冷导入根因维持既有未解。
- 当前记录密度.8928303～1.1026058、半均值RMS.902615，不把边界由约束匹配称为内部稳定；未据此追加作业或物理模型。保留分支和宿主管理工作树，不自动合并/推送。
