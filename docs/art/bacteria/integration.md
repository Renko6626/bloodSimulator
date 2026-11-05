# 细菌美术阶段集成

2026-11-05。用户明确要求提交推送、合并回main。实现提交 `d2a756f`（`feat: add textured bacteria with procedural flagella and capsules`）已从 `feat/bacteria-art` 快进纳入主目录main；收尾文档另作提交。本轮推送main与功能分支，保留功能工作树。

## 内容与一致性

包含两张内部纹理、程序贴图、2.6∶1圆端杆形与等面积归一化、真实外接半径生成余量、两条装饰鞭毛及半透明斑驳荚膜。沿用现有软体、接触战斗和snapshot接口，没有新依赖、shader或机器人改动。

主目录此前已同步未提交代码。集成前将主目录最新文档纳入功能分支，并核对全部50个修改／未跟踪文件；快进后逐文件哈希复核全部保留。`git diff --exit-code feat/bacteria-art --`通过，确认集成树一致。没有冲突或合并时额外产品代码调整。

## 验证

- 主目录 `bash scripts/build.sh`、`cargo fmt --all --check`、`git diff --check`通过。
- 主目录 `godot --headless --path godot --editor --import --quit`通过；`godot --headless --path godot --fixed-fps 60 --quit-after 10 -- --combat-preview`通过。日志在 `main-integration/`，已检查无脚本／引擎错误。
- 本阶段23项图像SHA256核对通过，生产纹理与候选相同。
- 沿用实现阶段60单元＋5集成、杆形最终4项cell＋12项immunity、已有GL网格、180帧战斗加载、静态截图及三次对应增量的只读审阅证据；没有因集成重复整套模拟测试。
- 主目录同步备份先临时stash，确认全部内容已在提交及合并树中后再清理本轮临时stash；不操作其他stash。

## 保留与边界

功能分支和宿主管理工作树保留。入口 `bash scripts/run.sh -- --combat-preview`；普通世界不新增细菌种群。生成草图、候选、旧截图和最终截图分目录保留。死亡／形变样板仅用于显示检查，不称实际战斗流程证据。

机器人、完整战斗FPS、人工手感和极端凹折仍暂缓／未验收。本轮没有性能、烘焙或云实验。
