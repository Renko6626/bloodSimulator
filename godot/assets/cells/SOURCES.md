# 血细胞纹理来源

2026-11-04。第一轮红细胞 / 中性粒细胞样板。

## 输入照片

- 文件：`docs/art/microscopy/sources/neutrophil-leonardi.jpg`
- 作者：Tommaso Leonardi；拍摄日期：2005-10-30。
- 原图页面：https://commons.wikimedia.org/wiki/File:Neutrophil.jpg
- 下载地址：https://upload.wikimedia.org/wikipedia/commons/f/f8/Neutrophil.jpg
- 页面记录为作者本人作品，作者声明全球 public domain；无法放弃版权的地区授权任何目的使用。
- 2026-11-04 核对页面并下载，原始照片保留署名，不作为整张游戏贴图直接使用。

## 产物与加工

`erythrocyte.png`、`neutrophil.png` 由内置 imagegen 根据上述真实照片重建并调整配色；不是原始显微像素的直接裁切，也不是医学测量数据。没有使用其他外部照片的像素。

红细胞参考照片中周围红细胞的中央淡染与颗粒；中性粒细胞参考照片中央的三叶连接核与胞质。去除原图黄偏色、背景、邻近细胞、署名与光照不均，用连续胞质填满方形四角，供网格裁切；外膜轮廓由游戏绘制。

提示词记录：

- 红细胞：以公有领域真实涂片作源图，重建单个红细胞内部纹理；中等饱和度的柔和砖红/珊瑚红，中央浅象牙玫瑰色淡染直径约图宽38%，平滑过渡到深色外围；保留细微显微斑驳。整个方形包括四角充满细胞材质，无圆形剪影、背景、透明、外轮廓、阴影、邻近细胞、文字或细胞核，无3D高光。
- 中性粒细胞：以原图中央细胞为参考，重建浅灰紫颗粒胞质与较深灰紫三叶核，三叶以细桥相连且有染色质纹理；核限定在中央区域，四周及四角填满胞质；去除黄偏色。无圆形外轮廓、背景、透明、红细胞、文字、阴影或大气泡，保留克制的显微颗粒。

两张纹理均不透明，通过节点固定 UV 随网格变形；核本轮烘在中性粒纹理中，未加入独立细胞器运动或状态。


## 第二轮：核与胞质分层

用户否定首轮整图拉伸后，通过内置 imagegen 编辑原 `neutrophil.png`：

- `neutrophil-cytoplasm.png`：提示移除全部三叶核与连接桥，用同色胞质无缝补全，降低颗粒对比和密度；全图不透明，无核、边界或大空泡。
- `neutrophil-nucleus.png`：提示只提取三叶核及连接桥，保留染色质纹理和轮廓，移除胞质及游离颗粒，包括核叶间空隙；真正透明背景，不添加阴影或膜。

原整图仅保留作为已否定版本的来源，不再被 World 加载。两张新图在渲染时分别绘制，未添加新的外部照片。


## 第三轮：斑驳与排列变体

新增参考原图：`docs/art/microscopy/sources/segmented-neutrophil-makysm.jpg`，作者 Makysm，来源 https://commons.wikimedia.org/wiki/File:Segmented_neutrophil.jpg ，页面声明 CC0；原图 https://upload.wikimedia.org/wikipedia/commons/1/16/Segmented_neutrophil.jpg 。已查看实际照片。

继续使用内置 imagegen 编辑/重建：

- `neutrophil-cytoplasm-v3.png`：以新真实照片的中央中性粒胞质为参考，要求显著增强宽窄不同的灰紫/米白不规则斑块与颗粒聚集，约8～15块跨图宽，避免均匀噪声；去除核、外膜与其他细胞，完整不透明方形，要求可平铺。平铺接缝未作专门验收。
- `neutrophil-nucleus-b.png`：以原透明核和真实照片作参考，保留3叶、连接桥与染色质纹理，将下叶稍移近左叶并转向、右上叶略向内，保持相近边距和整体尺度；真正透明背景。
- `neutrophil-nucleus-c.png`：相同参考与材质，改为更不对称的弯曲链状三叶，部分核叶靠近、一个稍拉长，不是简单旋转原图；保持透明背景与相近尺度。

上述是AI辅助纹理重建，不是显微像素的直接剪裁。上一版文件保留为历史来源。
