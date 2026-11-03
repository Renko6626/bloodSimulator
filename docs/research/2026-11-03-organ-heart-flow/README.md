# 心脏附近背景场：预检与首次作业现场

2026-11-03，工作分支`feat/organ-heart-flow`，宿主管理工作树`7c02`。用户已确认[设计](../../superpowers/specs/2026-11-03-organ-traffic-design.md)，[执行计划](../../superpowers/plans/2026-11-03-organ-heart-flow.md)先做心脏烘焙，区域外解析，再接细胞。

## 当前状态

已完成新例子`organ_heart_bake`、实际地图CPU预检和首次有界软件作业尝试。首次进程退出143（SIGTERM），没有输出`bake_report`或资产，终止原因未确定。尚未获得可用心脏背景，不接运行入口或细胞，不把预检或编译通过当作流场验收。

首轮之后补充了来源边界CPU扫描及离线作业阶段/进度日志，没有改变数值算法、地图、源标签、端口容差或预算；重跑等待用户确认。本机无硬件GPU，不启动云实验。

## 实际生产几何预检

- 标签矩形`[432,592]..[656,848]`，x=27..40、y=37..52，共224个16单位块，引用同一背景资产；不是224个常驻现场或求解器。
- 四个来源交接截面为(528,592)、(562,592)、(432,616.727273)、(656,620.24)。四腔包围框完整位于标签区，外围按<=0.25单位扫描仅找到四个声明端口。
- 首个方域`[416,576]..[736,896]`被生产规则拒绝。定位为右边界穿过外围edge55的(740,670)附近折弯；只补充拒绝位置，不放宽规则。
- 修订方域`[416,584]..[672,840]`、h=.5、512²通过。仅两套心脏连通流体区域，分别41146/41683个流体格，四个开口；各区域输入输出Q均约48.000004，收支为0。
- 生产参考压差14.145665169，循环Q48；最大初场世界速度7.9999495，格子速度.039999746，低于.05预算。

覆盖图橙线为离线方域，青线为预计算标签范围，白点为运行交接截面。它只表达真实几何和来源边界，不是已求得的血流。

![烘焙区域与四个接缝](attempt-01/region.png)

## 首次实际作业

`dt=.0025`、每逻辑帧16步，1500帧预热＋1500帧采样、stride25；计划60＋60模拟秒、60样本、48000格子步。外层墙钟上限1200秒，TERM后10秒强制结束。此为显式本地软件功能/资产作业，无实卡/FPS结论。

```sh
VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json \
timeout --signal=TERM --kill-after=10s 1200s \
  /data/sunyunbo/www/bloodSimulator/target/release/examples/organ_heart_bake \
  --bake --allow-software --output /tmp/sunyunbo/organ-heart-flow-bake
```

报告文件修改时间为2026-11-03 06:49:09 UTC；06:57:56 UTC轮询确认退出143，没有最终报告。没有时间戳或信号来源证据能确定实际退出时刻/原因，不能宣称是20分钟timeout、数值发散或OOM。

首次二进制SHA256为`fa8cabdf6a5aa0f30ce6c45ad8de9773af1058b4341d18a765f8336ede58680d`。实际作业使用的[例子源码](attempt-01/job-source.rs)和[几何实现](attempt-01/lbm-geometry-source.rs)已单独保存；最终例子后来补充了来源边界CPU扫描，不混称为首次二进制的源码。新增离线进度日志也未包含在首次二进制内。

## 已有检查

- 基线`cargo test --locked -p simulation --quiet`：50单元＋5集成通过。
- `cargo test --locked -p simulation --features lbm-gpu --quiet`：63单元＋5集成通过，10项GPU诊断忽略。
- `cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu`：通过；增加日志后的全目标编译也通过。
- `cargo fmt --all --check`：通过。
- `cargo test --locked -p blood-fluid-gpu lbm:: --lib --quiet`：7项通过、6项忽略。

未进行新入口Godot加载、实际四口资产导入、心腔近壁/流线检查、细胞通行、手感、实卡性能或云实验。日志和停止决策见[执行记录](controller-notes.md)。
