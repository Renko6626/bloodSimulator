# 细胞美术纳入主目录

用户已认可最新效果，要求将已确认的白细胞与红细胞表现纳入游戏，随后处理血管与外部组织背景。

## 纳入范围

- 红细胞的显微质感纹理。
- 中性粒细胞独立胞质和核，增强胞质斑驳、三种核排列和按ID固定角度，核仅作显示。
- World细胞平滑显示轮廓；中性粒与巨噬细胞轻微不规则的物理参考形状，面积、节点数与材料不变。
- 默认入口和战斗预览直接使用，无新增美术开关。巨噬细胞内部纹理仍待制作；lab维持诊断画法。

## 已有证据

59单元+5集成、workspace检查、独立桥构建、战斗/stream短加载、真实截图与最终整合审阅通过；不重复整套检查。人工视觉方向已获用户认可，不外推为FPS或全地图手感验收。

功能分支 `feat/cell-microscopy-textures` 保留。主目录集成使用快进，提交后另记录实际结果；不推送远端。


## 集成结果

实现提交 `2b27b14` 已快进纳入主工作树main，无冲突。先前主目录两份未跟踪美术笔记已核对并备份到 `/tmp/sunyunbo/cell-texture/main-art-before-integration`，然后采用功能分支的完整记录。

主目录 `bash scripts/build.sh`、Godot headless editor import、默认入口与 `--combat-preview` 各180帧加载、fmt与diff检查通过，运行桥库包含非圆参考形状。沿用功能分支59+5和审阅证据，没有重复全套测试。未推送。

主目录启动：`bash scripts/run.sh`；战斗预览：`bash scripts/run.sh -- --combat-preview`。

后续背景首版在独立分支 `feat/microscopy-background` 中准备，已确认细胞功能分支保留。
