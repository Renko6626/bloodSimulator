# 实施取舍与边界

用户已选择独立坐标导出，目标是地图细节开发时快速查看当前真实几何。以下为执行账本全部决定及代价，按发生顺序保留。

1. Ruling: implement the explicitly selected coordinate CLI without another approval round — already proposed and confirmed; cost is reversible example-only tooling if misunderstood.

2. Ruling: reuse host isolated worktree with new branch based on currentmain, preserving prior branch refs — no main mutation; cost is user must switch/merge later to use the new CLI from main.

3. Ruling: shared geometry writer stays in examples/support and only serves two existing needs — no public renderer or new dependency; cost is not a general-purpose map editor.

4. Ruling: SVG is tool output, PNG uses the existing external preview conversion — no new image backend; cost is PNG convenience depends on existing local conversion tooling.


5. Ruling: leave positive-subnormal rectangle scale overflow as an explicit unsupported numeric-extreme limit — current map coordinates/features have no meaningful use for1e-310 selections, low-frequency invalid output is observable/regenerable; cost is such extreme input may need a meaningful larger rectangle or future focused fix, no numerical framework now.

## 使用与验证边界

- 新命令直接构造地图定义，不创建模拟、图求解、物理世界或Godot；旧探针仅抽取相同绘图片段，原有CSV/固定三图/Q提示/移动守卫保持。
- SVG原生根视口裁切保留穿过选区但端点在外的血管。显式clipPath曾触发本机MSVG兼容问题，简化后现有convert能显示几何；归档PNG用系统已有librsvg/cairo外部转换，未安装或集成新后端。
- 可选标注按当前管段索引和内径显示，不做避让，可能重叠或被边缘裁切；索引随地图定义修改可能变化，心腔内部图边的D值也不等于腔室整体宽度。
- 实际心脏/微管/小地图及极小跨越视口示例、XML解析、3项例子测试、fmt和all-targetfeature编译已经完成；没有长回放、全量模拟套件、Godot、GPU、云或性能实验。
- 当前功能保存在feat/map-region-render及宿主管理工作树；本轮仅本地提交，不推送或合并，不改变main地图和游戏行为。
