# SDD ledger — plan: docs/superpowers/plans/2026-11-03-heart-boundary-alignment.md

用户确认烘焙边界和实际交接同面、四口使用同一解析剖面，暂不加混合段。沿用feat/organ-heart-flow工作树，起点579e0b4。

- Pre-flight: Task 1提供显式剖面和对齐导出及元数据；Task 2用同一例子运行原校验，只有通过才执行Task 3嵌入，不存在接口冲突。
- 设计builder检查nothing important left。实施坐标y656是已确认路线内的几何选择；未引入新求解器/格式/来源类型。
- Task 1 CPU测试：boundary_export_keeps_actual_interior_and_prescribed_trace先因未实现返回Err失败，完成后offline_lbm三项均通过。测试直接调用真实导出/背景采样，独立期望边界1.5、内部(.75,.125)、壁面0和拒绝未声明支撑流体。
- Task 1软件GPU小用例：bake_applies_supplied_profile_before_advancing先因仍使用默认剖面得到.14651163而失败，接入原setter后通过（完整作业约.15秒）。这是16²/12格子步关键接口功能验证，不是另一轮心腔烘焙或性能实验。
- Task 1真实CPU预检通过：384²，标签[464,656]..[608,848]，四腔完整；四口沿程10/80.93172/10/86.65863，原2r检查通过。两个流体分量28041/28693格，各分量Q收支0，初场格子峰值<.05。
- Ruling: 导出增加一排域外支撑并在真实边界写出已施加的连续解析条件，内部只用真实原场双线性采样；不使用速度混合、内部归一化或修改原容差。离散法向通量归一和连续解析的O(h²)差别按原容差检查。
- 后续单次心腔作业仍为dt=.0025、16步/逻辑帧、1500预热+1500采样/stride25、48000格子步、60样本，墙钟1200秒。失败停止，不自动重跑或转云。
