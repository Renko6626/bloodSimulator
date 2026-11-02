# 按坐标导出地图

`map_render`只构造现有地图定义，导出心腔轮廓和真实管宽，不启动模拟或求解流量。坐标使用世界单位，Y向下，负坐标可直接传入。

```sh
CARGO_TARGET_DIR=/data/sunyunbo/www/bloodSimulator/target \
  cargo run --locked -p simulation --example map_render -- \
  --rect -48 -150 288 390 --grid 16 --labels --output /tmp/heart.svg
convert -background '#172333' /tmp/heart.svg /tmp/heart.png
```

`--rect X Y WIDTH HEIGHT`和`--output FILE.svg`必填；宽高须为有限正数。输出目录须已存在。默认地图为`heart`，可用`--map organ`切换至新器官路网，或`--map small-world`切换至既有小地图。`--help`显示参数说明；非法参数或文件写入失败会返回非零退出码。

`--grid STEP`可选，按世界原点对齐并显示坐标；STEP须为有限正数，间距至少约12像素且网格线最多200条，过密时会提示增大STEP。`--labels`可选，显示可见中心线上的管段索引、内径（D，世界单位）及心腔名称。标签不做自动避让，边缘文字可能被裁切，密集处可关掉标注后另导出更小范围。

长边约1200像素，保持世界比例。管道完整写入SVG，由根视口裁切；两端在选区外但经过选区的管段仍可见。颜色沿用旧诊断图，不能据此推断运行流向或流速。PNG转换由外部工具完成，CLI只写SVG。本机ImageMagick原生SVG读取器存在文字显示差异，优先查看SVG或使用librsvg转换。

已导出的[心脏、微管与小地图示例及检查记录](../research/2026-11-02-map-region-render/README.md)可供参考。

管段编号是当前地图定义的索引，修改地图后可能变化。心腔内图边的D值是该图边的直径，不代表整个心腔的通行宽度。工具按实际地图尺度使用，接近浮点精度极限的选区不在本次验收范围。

新路网的心脏区域可用`--map organ --rect 400 550 300 300 --output /tmp/organ-heart.svg`导出。可走图预览使用`bash scripts/run.sh -- --organ-preview`，旧图保留`--heart-preview`；两者互斥，均自动使用区块加载和zoom2，固定无流、无红白细胞。预览保留WASD、速度调节、Space暂停、N单步、R重置和C碰撞显示，禁用压力、细胞拖曳及F流向显示；不能与LBM、Stokes、profile或benchmark参数合用。
