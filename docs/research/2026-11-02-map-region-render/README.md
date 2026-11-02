# 坐标地图导出工具记录

2026-11-02，实施于`feat/map-region-render`，基于`bf44817`。范围为独立例子及文档；没有修改生产地图、运行时、Godot、Cargo或依赖。说明见[工具用法](../../tools/map-render.md)。

几何来自`crates/simulation/src/map.rs`的`MapDefinition::small_world`和`map/heart.rs`的`MapDefinition::heart_preview`（分级管道由`map/heart/vascular.rs`构造）。新命令直接调用地图工厂；绘图复用旧`heart_preview_probe`的多边形、折线、圆角、颜色及真实`half_width * 2`。共享文件仅位于`examples/support/map_svg.rs`，旧探针仍保留CSV、图流量提示、三个视图、域标记及原移动守卫。

## 实际导出

完整命令与退出结果在[exports.txt](exports.txt)。以下均带世界对齐网格和可见中心线管段标注；索引从0起，D为世界单位内径。SVG保留全图几何，由根视口裁切，PNG为外部预览转换。

| 选区 | 世界矩形 | SVG | PNG |
| --- | --- | --- | --- |
| 心脏 | -48 -150 288 390 | [heart.svg](heart.svg) | [heart.png](heart.png) |
| 肺部微管 | -140 -1330 280 150 | [micro.svg](micro.svg) | [micro.png](micro.png) |
| 既有小地图 | -45 -30 90 60 | [small-world.svg](small-world.svg) | [small-world.png](small-world.png) |

PNG使用系统已有`/usr/bin/python3`、PyGObject、cairo及librsvg 2.52生成，无安装或项目依赖变化，XML解析及像素尺寸见[preview-checks.txt](preview-checks.txt)。重建预览可在仓库根目录运行：

```sh
/usr/bin/python3 - <<'PY'
import gi, cairo, math, xml.etree.ElementTree as ET
from pathlib import Path
gi.require_version('Rsvg', '2.0')
from gi.repository import Rsvg
for path in Path('docs/research/2026-11-02-map-region-render').glob('*.svg'):
    root = ET.parse(path).getroot()
    w, h = float(root.attrib['width']), float(root.attrib['height'])
    surface = cairo.ImageSurface(cairo.FORMAT_ARGB32, math.ceil(w), math.ceil(h))
    viewport = Rsvg.Rectangle()
    viewport.x = viewport.y = 0
    viewport.width, viewport.height = w, h
    assert Rsvg.Handle.new_from_file(str(path)).render_document(cairo.Context(surface), viewport)
    surface.write_to_png(str(path.with_suffix('.png')))
PY
```

本机ImageMagick的MSVG读取器在早期显式clipPath版本中漏画几何，并对文字描边产生异常。最终版本改为根视口裁切、普通文字；`convert -background '#172333' heart.svg heart.png`可以显示几何，但本机MSVG仍有部分字形显示差异。归档PNG使用上述librsvg命令。

## 检查结果与边界

所有Cargo命令使用`CARGO_TARGET_DIR=/data/sunyunbo/www/bloodSimulator/target`：

- `cargo test --locked -p simulation --example map_render`：最终退出0，3项例子测试通过，覆盖负坐标/非法输入、跨视口管段与标签、原点对齐网格及文本转义。
- `cargo fmt --all --check`：最终退出0。
- `cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu`：最终退出0；仅编译，没有运行GPU后端。
- `cargo run --locked -p simulation --example heart_preview_probe -- .superpowers/sdd/2026-11-02-map-region-render/legacy-diagnostics --diagnostics-only`：退出0。viscosity=0.05、unit_rate=1.5853244、reference_pressure=30.277714、reference_rate=48；CSV为224边＋表头，三个SVG均可XML解析、含224条管段及Q提示。未进入穿行。
- 三个实际SVG均XML解析成功，静态查看librsvg PNG。另导出`--rect -1 -65 2 2 --labels`，原始产物已归档为[crossing.svg](crossing.svg)及[crossing.png](crossing.png)，XML和PNG检查显示223号D24管道覆盖选区，标签锚点(0,-64)位于可见中心线，两端均在选区外；原始管宽未被裁切改写。

静态观察：真实四腔轮廓和D24/12/6/3层级保留，小地图沿用自身D7/6.2；心脏左室标注与一条管段标注有重叠，部分边缘文字被裁切，符合简单标注、不做自动避让的范围。网格、标签可独立关闭。结果只表示地图几何展示，不表示流向、流速、运行画面、物理精度或性能验收。未执行默认模拟测试套件、长回放、Godot、GPU/云/性能实验、推送或合并。

逐任务和最终整分支审阅均通过，无待修复发现。见[任务审阅](task-review.md)、[最终审阅](review.md)及[执行取舍](controller-notes.md)。本地工作树为 `/data/sunyunbo/.codex/worktrees/4aad/bloodSimulator`，分支 `feat/map-region-render`；本轮不推送或合并。
