# 静态血管背景缓存实现与本地核对

2026-10-29，分支 `feat/godot-static-background`。用户已在选择方案1后明确安排实施。静态血管纹理缓存已接入 Godot；本地检查与截图核对完成，RTX3060性能对照尚未执行。实现期间没有启动/恢复云实例；随后用户已安排文档、提交与推送，见[阶段交接](../2026-10-29-performance-checkpoint.md)。

## 实现

- `world_background.gd` 创建独立 World2D 的 SubViewport，使用 UPDATE_ONCE；Sprite2D 直接复用其 GPU 纹理。缓存为不透明 LDR、原点取整，保留8px边距。
- `world_background_painter.gd` 在加载时绘制地标背景、底板、地标框/名称和普通管壁；缓存范围合并血管 bounds 与地标/真实字体文字范围。
- World 的普通 `_draw` 只保留细胞、机器人及开启的 flow/collision 调试内容。动态画法和抗锯齿未变；reset、相机和调参不重建背景。
- headless 或禁用 render loop 时直接跳过背景资源分配，模拟照常推进。Rust、GDExtension字典、碰撞及LBM均未改。

新增代码只有两个 Godot 脚本及 UID，修改 world.gd/world.tscn；[设计](../../superpowers/specs/2026-10-29-static-background-design.md)和[计划](../../superpowers/plans/2026-10-29-static-background.md)已同步状态。

## 画面与渲染工作量

同机 Xvfb＋Mesa llvmpipe、1280×800、Godot4.7.1，均在第120步、同一输入/相同相机位置截图。没有键鼠输入或UI自动交互。默认地图两侧使用同一份原release bridge；验证场两侧使用上一轮已封存的诊断bridge，以 `BLOOD_ALLOW_SOFTWARE=1` 显式允许软件适配器，仅作画面核对。软件渲染结果不能作为RTX性能成绩。

| 场景 | 缓存大小 / 原点 | baseline draw calls / 图元 | 缓存 draw calls / 图元 |
| --- | --- | --- | --- |
| small_world | 2244×1217 / (-901,-421) | 416 / 171112 | 401 / 8730 |
| LBM验证场 | 3780×3206 / (-1290,244) | 412 / 371742 | 399 / 8434 |

这是 RenderingServer 实际稳定帧计数。图元数和三角形数不是可任意混用的指标，整场draw calls也没有变成1；动态细胞仍保留大量绘制提交。

已查看缓存前后的[默认地图](local-checks/cached.png)和[验证场](local-checks/validation-cached.png)截图，未见地标裁切、黑块、重复对象或明显轮廓偏移。世界区域排除右侧 timing HUD 后，逐通道平均像素绝对差分别约0.029/255、0.046/255；不是逐像素相等证明，也不以图片差异代替人工看图。原图、计数和[像素差记录](local-checks/pixel-comparison.json)保留。

## UPDATE_ONCE 核验口径修正

最初计划要求稳定后读取节点属性为 DISABLED，这一断言不适用于Godot4.7：

- [SubViewport::get_update_mode](https://github.com/godotengine/godot/blob/4.7-stable/scene/main/viewport.cpp#L5256)返回节点缓存成员，实际截图时仍为 UPDATE_ONCE=1。
- [渲染后端](https://github.com/godotengine/godot/blob/4.7-stable/servers/rendering/renderer_viewport.cpp#L872)在一次绘制后直接把内部模式切换为 DISABLED，不回写节点属性。

因此没有为“让getter变成0”添加多余回调；源码契约、纹理实际显示、稳定帧图元中不再包含静态几何共同支持缓存生效。对外报告明确区分Node属性与后端状态。

## 实际检查

```bash
XDG_DATA_HOME=/tmp/sunyunbo/blood-static-implementation/userdata XDG_CONFIG_HOME=/tmp/sunyunbo/blood-static-implementation/config bash scripts/check.sh
```

exit0：格式、workspace编译、现有simulation测试、bridge构建、cached editor import、world与lab的headless运行均通过。[完整日志](local-checks/project-check.log)。本轮未复现已知的首次空缓存import134，也不宣称修复该异常。

```bash
for script in world_background_painter world_background world; do
  XDG_DATA_HOME=/tmp/sunyunbo/blood-static-implementation/userdata XDG_CONFIG_HOME=/tmp/sunyunbo/blood-static-implementation/config godot --headless --path godot --script "res://$script.gd" --check-only
done
git diff --check
```

全部exit0。没有新增永久测试或独立验证工程；临时截图/只读状态查询仅放在 `/tmp/sunyunbo/blood-static-implementation/`。

实际推进检查使用临时SceneTree入口加载真实world场景，等待180个physics信号后读取现有状态，不发送输入：headless＋disable-render-loop、X11/GL＋disable-render-loop两例均exit0，记录为179步、cache_allocated=false，见[headless日志](local-checks/headless-progress.log)及[GL无绘制日志](local-checks/gl-no-render-progress.log)。这保证检查不是只看退出码。

四个截图运行的公共命令：

```bash
DISPLAY=:197 LIBGL_ALWAYS_SOFTWARE=1 STATIC_CAPTURE=/tmp/sunyunbo/blood-static-implementation/cached.png XDG_DATA_HOME=/tmp/sunyunbo/blood-static-implementation/userdata XDG_CONFIG_HOME=/tmp/sunyunbo/blood-static-implementation/config godot --path /tmp/sunyunbo/blood-static-implementation/cached/godot --fixed-fps 60 --disable-vsync --audio-driver Dummy --script /tmp/sunyunbo/blood-static-implementation/capture.gd
```

baseline替换项目路径/输出文件；验证场使用validation-baseline或validation-cached路径，增加 `BLOOD_ALLOW_SOFTWARE=1 XDG_RUNTIME_DIR=/tmp/sunyunbo/lbm-profile-20261029/runtime` 和用户参数 `-- --local-lbm --lbm-validation`。完整输出保存在local-checks/capture-*.log；初次截图时的Xvfb进程提前退出导致一次X11启动失败，重启独立前台Xvfb后四例均正常退出，不把那次启动失败计为截图通过。

独立只读审阅覆盖四个产品文件、新增脚本、坐标/地标范围、独立World2D和无绘制分支，无阻断或minor发现；没有据此宣称已获得实卡性能收益。

## 尚未验证

RTX3060同机同负载的缓存前后帧间隔、GPU时钟/LBM等待、冷启动首帧耗时及实际总显存尚未测量。前次profile的no-World控制同时跳过动态对象，不作为本次缓存后FPS预测。普通管壁仍在加载时使用原细分几何，启动几何生成成本也未消除。

暂停/单步/reset/flow/collision的人工操控手感未验收，窗口大幅放大后的清晰度未验收。代码保留原调用路径，未运行自动UI交互。

## 后续已授权：细胞视野剔除

用户随后要求实现视野剔除。World 使用真实 canvas/world 逆变换得到当前视口矩形，换算回世界单位并留8px边距；以每个细胞真实软体节点的AABB判断相交，在构造缩放数组和多边形前跳过屏幕外对象。没有改 snapshot、软体材料或物理推进；当前仍遍历全部细胞做轻量包围盒检查。

默认场景同一release库、同帧120的对照：在静态缓存基础上，draw calls401→146、图元8730→4038；排除timing HUD的800000个世界像素完全一致，玩家位置相同。见[截图](cell-culling/culled.png)、[计数](cell-culling/capture.log)、[像素对比](cell-culling/pixel-comparison.json)。这是本机llvmpipe截图证据，不是实卡FPS成绩。world.gd check-only、headless180退出均为0；独立审阅未发现坐标/边缘误裁剪问题。此局部修改未重复运行未受影响的Rust全量检查。

同时用户选定了后续细胞模拟窗口化的“远场数量/密度，近场真实软体”方向；那是独立物理调度/状态模型，当前Godot剔除不实现或暗中替代该模型。
