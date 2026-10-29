# 性能优化阶段归档

2026-10-29。用户要求“阶段性写文档和提交推送”。本阶段统一保存在 `feat/godot-static-background`，基线为 `41057e3`；本次安排是文档、提交和推送，不包含合并到 main、启动下一轮优化或云实验。

## 已完成的工作

| 工作 | 结果与证据 |
| --- | --- |
| RTX3060游戏帧定位 | tick桥接约6–7μs，主要问题在World绘制路径与伴随的LBM执行/完成等待；正常复测帧间隔均值66.75ms，诊断副本关闭线条AA为36.80ms。见[完整定位](2026-10-29-gameplay-profile/README.md)。 |
| 静态血管背景缓存 | 普通底板、管壁、地标在加载时烘焙一次并显示纹理；保留动态细胞和调试画法。默认场景稳定帧图元171112→8730；软件截图已核对。见[实现记录](2026-10-29-static-background/README.md)。 |
| 细胞视野剔除 | 依据真实软体AABB和相机视口，在构造绘图数组之前跳过屏幕外细胞。默认同帧draw calls401→146，世界区域像素一致；这是绘制剔除，不改变物理。见[截图与计数](2026-10-29-static-background/README.md#后续已授权细胞视野剔除)。 |
| 近场软体与远场数量 | 显式 `--local-cells`，72×72激活、80×80退休、32～36外缘构造、每0.2模拟秒按图流输运数量。一个Rapier世界，退休真正删除软体。见[设计](../superpowers/specs/2026-10-29-cell-population-window-design.md)、[实现和原始数据](2026-10-29-cell-population-window/README.md)。 |

CPU真实MoveInput回放各60模拟秒：全量72个软体，Local为28–55个；生成66次、退休68次，最大数量误差2.84e-14。本机解析流模拟均值6.335→3.755ms，p95 7.552→4.684ms；交接帧整步p95为4.991ms。完整数据保留95.867/45.504ms的未归因长尾。此结果改变了远场物理近似，不能外推为RTX/LBM或完整游戏帧率收益。

## 当前运行方式与模型边界

默认世界继续全量软体与解析流，静态缓存和屏幕绘制剔除默认生效。`--local-cells`只支持world和既有LBM验证场，loop lab与local Stokes组合明确拒绝。

```bash
bash scripts/build.sh
godot --path godot -- --local-cells

bash scripts/build.sh --features lbm-gpu
godot --path godot -- --local-lbm --lbm-validation --local-cells
```

近场保留原十二节点软体材料、接触和机械子步；远场不保存普通红细胞个体身份、位置和变形。未实体化库存包含窗外数量与窗内受阻/不足整数的数量，总量账只有一份。初始分布沿用旧world/validation种子。群体模式不再输出依赖固定身份的速度和累计位移成绩。

后续讨论了按实体用途选择远处冻结、低频更新或直接卸载。目前只实施普通细胞数量模型；任务对象/敌人等的持久状态与生命周期没有设计定案或新增实现，也不引用Minecraft/GTA具体机制作为本项目的技术事实。

## 验证与尚未验收

提交前重新执行 `bash scripts/check.sh`，覆盖格式、workspace编译、simulation现有单元/集成检查、bridge构建、cached editor import、world/lab headless运行；另跑 `cargo test --locked -p simulation --features lbm-gpu --lib` 和feature bridge编译。本次复查exit0：默认16个单元/5个集成测试通过；feature lib为23通过、9个显式GPU忽略；feature bridge编译通过。见[默认项目日志](2026-10-29-cell-population-window/local-checks/archive-project-check.log)、[feature测试日志](2026-10-29-cell-population-window/local-checks/archive-feature-tests.log)与[feature编译日志](2026-10-29-cell-population-window/local-checks/archive-feature-bridge.log)。

原始日志和诊断补丁按字节保留，未清洗末尾空行、CRLF或diff上下文空格；源码、脚本和文档单独通过 `git diff --cached --check`。

既有验证还包括CPU实际生成/退休、受阻库存、暂停/单步/reset和反向流候选顺序；Full/Local各2帧llvmpipe LBM功能短测；Godot默认场景同帧截图。没有自动UI键鼠、点击或表单交互。

以下项目保留为待验收，不将归档视为已解决：

- 优化后RTX3060同机的Rapier/背景采样/LBM及完整渲染帧对照，GPU等待和降频的进一步归因。
- LBM移窗与细胞生成/退休同时发生的联合长测，人工推挤、切换边界及暂停/reset手感。
- 冷启动首帧、缓存实际总显存、变焦或超宽视口；当前窗口尺寸仅覆盖现有视口和48×48 LBM。
- 移窗prepare尖峰、首次空缓存editor退出134；cached import通过不等于根因已修复。
- 默认small_world的LBM开口限制、严格堵塞、全局反压与远场密度团精确旅行时间。

## 云端状态和继续工作的入口

最近硬件定位实例 `58212504` 已取回并核对结果，平台确认 `cur_state=stopped`、`actual_status=exited`，见[停机回执](2026-10-29-gameplay-profile/hardware-rtx3060/final-stop-status.json)。此前 `58133143` 也已停止。本轮本地实现和归档没有恢复或新租实例；保留磁盘仍可能有存储费用。

继续工作先读[STATUS](../STATUS.md)及上述三个实施/研究报告。提交、推送或报告中的未勾选验证项，不构成自动启动下一轮优化、云实验或改用另一种远场实体模型的授权。
