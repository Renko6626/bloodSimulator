# 阶段集成记录

用户明确要求整理文档、阶段性合并main、提交并推送。本阶段已完成本地快进集成，无冲突或合并时产品代码调整；按本次授权推送main。

## 提交

- `2b27b14`：细胞显微材质、核/胞质分层、固定外观差异与白细胞非圆参考形状。
- `531c809`：细胞阶段纳入主目录的记录。
- `dcb2a60`：内皮管壁、组织背景、专用管腔层次、红细胞第二版，以及统一后的美术文档。
- 本页及STATUS/AGENTS的最终集成状态另作收尾文档提交，一并推送。

`feat/microscopy-background` 已从 `531c809` 快进纳入主目录main。合并后、收尾文档更新前，`git diff --exit-code feat/microscopy-background --` 通过，确认代码树一致。主目录此前已构建包含细胞物理改动的运行桥库；本轮背景阶段没有Rust产品代码变化。

## 收尾检查

- `cargo fmt --all --check`、`git diff --check`：通过。
- 两份素材SHA256清单全部核对通过，共13项。
- 主目录 `godot --headless --path godot --editor --import --quit`：退出0、无脚本错误，四张新纹理已导入。
- 主目录 `godot --headless --path godot --fixed-fps 60 --quit-after 10 -- --combat-preview`：退出0、无脚本错误，仅作合并后的短启动检查。
- 最终只读定点审阅无重要缺陷，涵盖管腔世界UV、墙边8px淡出/padding、顶点属性与素材引用。

沿用已有59单元+5集成、workspace检查、桥构建、网格行为与多场景加载/真实截图证据，没有重复整套验证或新增永久测试。

## 保留与边界

功能分支及宿主管理工作树保留，只推送main。既有细胞和背景迭代图片保留作历史证据。未启动新云实验、烘焙或性能研究。FPS、全地图人工手感、极端形变及像素级无缝尚未验收；巨噬内部、细菌与机器人仍有占位表现。

统一入口：[阶段记录](README.md)。主目录运行 `bash scripts/run.sh`，战斗预览为 `bash scripts/run.sh -- --combat-preview`。
