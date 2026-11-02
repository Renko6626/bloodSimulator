# 来源与复现

工作位置：`/data/sunyunbo/.codex/worktrees/4aad/bloodSimulator`，分支`feat/organ-map-preview`。Task3基线`7f3d647`（Task1地图`47597b8`、Task2入口`7f3d647`）。源设计为[候选A](../../superpowers/specs/2026-11-02-playable-vessel-network-design.md)与[接入方案](../../superpowers/specs/2026-11-02-playable-map-integration-design.md)。

所有Cargo命令设置：

```sh
export CARGO_TARGET_DIR=/data/sunyunbo/www/bloodSimulator/target
```

## 有限诊断

```sh
cargo run --locked -p simulation --example organ_preview_probe -- docs/research/2026-11-02-playable-map-integration
```

探针自身声明全部连续路线和总25000步/每段2400步上限。先对全部计划路段按不大于0.1单位采样检查生产SDF，再构造生产Simulation执行真实MoveInput。默认dt=1/60、诊断speed=8（沿用旧探针；不修改游戏默认速度）、生产身体radius=0.85，中心净空守卫>=1.0；到点误差<0.1。A/B/C只在构造前更改MapDefinition.spawn，运行中没有瞬移。记录为[probe.log](logs/probe.log)。

同一例子直接调用`GraphNetwork::new(&MapDefinition::organ_world(), Config::default().viscosity).solve(1.)`输出[unit-graph.csv](unit-graph.csv)，包含实际边端点/坐标、直径、长度、有符号Q、Q/泵Q、Q/D、压强及黏度/驱动。Q/D仅统一平面通量诊断，未指定二维深度，不当作心腔真实速度场。没有解析测试日志、逐边赋速或使用旧Q48。

## 实际地图导出

```sh
cargo run --locked -p simulation --example map_render -- --map organ --rect 100 60 960 1070 --grid 80 --output docs/research/2026-11-02-playable-map-integration/overview.svg
cargo run --locked -p simulation --example map_render -- --map organ --rect 370 60 370 800 --grid 40 --labels --output docs/research/2026-11-02-playable-map-integration/heart-lung.svg
/usr/bin/python3 docs/research/2026-11-02-playable-map-integration/render_png.py
```

实际首轮导出调用相同target下已构建的`debug/examples/map_render`可执行文件，参数如上。坐标世界单位、Y向下；SVG几何直接取真实工厂。Python仅加明确的零流标题并通过系统GI Rsvg2/cairo转换PNG，没有构造/替代地图几何。不装依赖、不使用ImageMagick。首轮转换时错误解包Rsvg intrinsic-size返回值退出1，移除无用调用后转换退出0，SVG几何未受影响。全景1076×1200，心肺555×1200；主agent已静态查看，标签重叠属现有工具限制。

## 集中检查

完整输出和退出码保存在[logs](logs/)。`check.log`包含默认检查、构建及Godot import/world/lab；feature/全目标编译和预览加载分别记录，不重复默认suite。不启动浏览器自动按键/点击，不重跑旧211点巡游。

探针回放后仅将已有零态断言合并并添加case/segment/pose/clearance失败上下文，守卫条件与成功行为未变化；补跑`cargo check --locked -p simulation --example organ_preview_probe`退出0，fmt/diff退出0，没有重复整套回放。
