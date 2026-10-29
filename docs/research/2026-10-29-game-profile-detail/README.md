# 当前LBM＋细胞窗口的帧内分解

2026-10-29，用户明确要求进一步拆分上轮约5.38ms帧间隔剩余及LBM约2.08ms完成等待。基线d0dfa07，不修改生产模拟/渲染行为、不采用10步试验，不自动提交推送。

## 安排

- 恢复同一RTX3060实例58263433，沿用既有1美元预算；结果核对后停止、保留磁盘。
- 相同分叉场、Local细胞、Fine192²/14格子步、四线程物理、GL compatibility、1280×800、禁用vsync，120预热+3600测量步移动回放。
- 正常生产、CPU细分、CPU+Godot渲染计时、再加wgpu compute时间戳四组，量化测量干预；不在性能主测中截图或新增同步glFinish。
- 脚本计时：完整physics callback、CSV、snapshot/bridge、HUD、前次_draw、process callback、process结束到frame_pre_draw、frame_pre/post_draw跨度。核对pre/post帧标签后才合并墙钟闭合账。
- 渲染计时：Godot原生frame setup CPU、viewport CPU/GPU；GPU结果有延迟，只统计稳态区间，不硬配给同编号的尖峰。源码确认Compatibility使用GL timestamp并在约两帧后取回，仍可能干预。
- LBM：已有upload/encoding/submission/readback等待/解码，加CPU均值统计、修正提交、缓存检查；以PROFILE_BEGIN/STEP界定帧，GPU query与现有读回合并，不额外提交或poll。等待与GPU pass存在包含/重叠关系，不直接求和。
- 如需因果控制，才追加有界no-world诊断（仅隐藏动态画法，物理不变）；不把隐藏画面叫游戏FPS。

## 结果

五组均完成120预热＋3600测量步，各3599个测量帧间隔；程序退出0、状态COMPLETE，14格子步、有限值、数量账通过，均实际移窗5次。玩家位置、密度最小/最大/总和、细胞数量和迁移计数在五组之间逐帧一致；这些字段也与上一轮Local回放一致。生产代码没有改动。

| 组别 | 帧间隔均值 ms | p95 ms | 最大 ms |
|---|---:|---:|---:|
| 正常生产版 | 20.673 | 26.709 | 38.085 |
| CPU分段计时 | 20.552 | 26.420 | 51.953 |
| 加Godot viewport计时 | 19.380 | 24.651 | 49.181 |
| 再加wgpu时间戳 | 20.822 | 28.143 | 52.239 |
| 补充主循环边界计时 | 15.179 | 18.565 | 30.295 |

同一机器的连续运行存在显著时间波动。前四组大量GPU采样位于P5/P8，显存频率中位数810MHz；补充组主要位于P5。正常生产版的源码、库和模拟轨迹与上一轮相同，整帧却从约15.13变为20.67ms；CPU阶段也变慢。GPU状态与差异有关，但没有CPU调度/频率的完整采样，不能将全部变化归因于GPU降频，也不能据一次顺序测量精确净扣计时干预。上述各组必须分别解读，不能挑最快一组当稳定FPS。

## 剩余帧时间落在哪里

补充组显式记录物理开始/结束、process开始/结束、render pre/post时间。全部3599帧的边界标签吻合，分解闭合误差小于0.003ms。其约15.18ms均值与上轮约15.13ms接近，适合展示阶段构成，但并非上轮5.38ms差值的逐帧追溯。

| 互不重复的墙钟分段 | 均值 ms | 占整帧 |
|---|---:|---:|
| Rust完整模拟 | 9.146 | 60.3% |
| 物理回调内其余部分：snapshot/HUD、CSV、桥接及守卫 | 1.063 | 7.0% |
| World动态绘图 `_draw()` | 1.786 | 11.8% |
| 物理回调结束到process，扣除上述World绘图 | 1.562 | 10.3% |
| render pre/post墙钟跨度 | 1.455 | 9.6% |
| process记录、render前处理、帧间其余部分 | 0.167 | 1.1% |
| 合计 | 15.179 | 100% |

物理回调内snapshot/bridge/HUD刷新约0.721ms，其中HUD脚本设置约0.103ms；CSV构造约0.154ms，均已包含在1.063ms中。Rust内LBM约4.936ms、Rapier2.783ms、prepare0.258ms、耦合0.212ms，其余约0.957ms。

本场景的World `_draw()`在物理后的消息队列刷新期间执行，结束距process开始仅约0.012ms；它不在render pre/post跨度中，也不在process结束到pre_draw的约0.103ms中。渲染结束到下一次物理开始仅约0.023ms。剩余1.562ms位于物理回调之后、process之前，这一区间还覆盖引擎物理/场景回调、延迟刷新、UI绘图与布局；当前证据不能将它全部命名为HUD或纯调度等待。

Godot viewport计时组中，viewport CPU均值1.584ms、GPU0.423ms；加wgpu时间戳组为1.724/0.427ms。它们是render墙钟跨度内部的测量，GPU结果还有约两帧延迟，不另加进上表，不能硬配给同编号尾帧。动态图形构造和渲染CPU提交值得继续优化；当前GPU像素绘制时间不足以解释数毫秒的全部外层成本。

当前每个可见红细胞仍逐帧构造外层多边形、内层多边形和抗锯齿轮廓，玩家还画圆和弧。流向箭头和碰撞调试默认关闭；静态血管背景沿用已烘焙纹理。此轮没有把全地图血管三角形重新提交作为解释。

## LBM读回与计算

补充组LBM均值4.936ms，其中既有读回等待约2.136ms，与上轮2.085ms接近。普通帧仍为33 dispatch、20 submit、3次读回，共2,506,752字节（约2.39MiB）；5次迁移帧还各多一次宏观场读回，不能误判为计时器额外同步。

| 既有同步位置 | 等待均值 ms | 用途 |
|---|---:|---|
| 中间宏观场 | 1.486 | 第一机械批之后供下一批耦合采样 |
| 原始9方向分布 | 0.241 | CPU统计帧末均值控制量 |
| 修正后宏观场 | 0.408 | 最终采样缓存与诊断 |

精确值见summary；表内包含迁移时序识别后的正常refresh读回，另有极小的迁移读回均摊。窗口可能在第一或第二机械批prepare时迁移，分析器用PROFILE_CACHE对应读回，不能一律将第一个/最后三个read当成正常批次。

CPU分项另测得upload约0.377ms、encoding0.845ms、submit0.569ms、读回解码/拷贝0.416ms、CPU均值统计0.237ms、两次缓存检查/主机处理0.283ms。工作计时包含prepare中的少量GPU操作，不能机械逐项凑成严格互斥的4.936ms。均值修正提交已包含在upload/encoding/submit中，CACHE.read也包含已有读回，不重复累加。

独立wgpu时间戳组测得全帧compute pass合计约3.203ms，分批GPU跨度合计约3.940ms；该组CPU读回等待约2.779ms。GPU计算可能与CPU编码、提交和机械计算重叠，跨度还包括批内空隙；两者不能相加或直接相减来推算“纯驱动耗时”。证据说明等待确实伴随可观的GPU工作，尚不能精确拆成PCIe拷贝、排队和驱动调度各多少。

## 结论与限制

下一步优化候选很明确：动态细胞几何构造/提交、物理后延迟处理，以及LBM每帧多次同步和CPU编码提交。中间宏观场是当前最大的显式等待点，不能仅凭本轮结果删除，因为它承接下一机械批的耦合采样。减少读回、改GPU归约或改变渲染接口仍需单独方案确认；本轮未实施优化或改变算法。

上一轮101ms尖峰未复现；本轮计时组最大约52ms，多数最大尾帧落在物理回调内。没有因此给旧101ms定因，也没有宣布尖峰已修复。单场景、固定分辨率、显式输入回放和禁用vsync不等于人工操作、战斗负载或桌面帧节奏验收。

## 复现与来源

从d0dfa07的git archive生成诊断副本，先运行[instrument.py](instrument.py)；它复用上一轮的基础插桩。补充组再运行[instrument-phases.py](instrument-phases.py)。五个诊断源码哈希已从干净archive复现。诊断release构建成功；[run-hardware.sh](run-hardware.sh)、[run-phases.sh](run-phases.sh)和[预跑](preflight.sh)保存精确开关。沿用[上一轮显示环境](../2026-10-29-game-e2e/README.md)，本次cached editor import退出0，启动仍提示缺libxkbcommon，未宣称首次冷导入问题已修复。

Godot 4.7.1官方源码确认了[viewport计时范围](https://github.com/godotengine/godot/blob/4.7.1-stable/servers/rendering/renderer_viewport.cpp)、[GL查询延迟取回](https://github.com/godotengine/godot/blob/4.7.1-stable/drivers/gles3/storage/utilities.cpp)及[主循环消息队列时序](https://github.com/godotengine/godot/blob/4.7.1-stable/main/main.cpp)。Performance.TIME_PROCESS约每秒报告该段最大值，原日志仅保留作参考，不拿它的均值闭合整帧。具体来源哈希见[源码记录](hardware-rtx3060/godot-source-references.json)。

```bash
python3 docs/research/2026-10-29-game-profile-detail/analyze.py \
  docs/research/2026-10-29-game-profile-detail/hardware-rtx3060/evidence/results
```

[五组汇总](hardware-rtx3060/evidence/results/summary.json)、[83个原始文件哈希](hardware-rtx3060/evidence/artifact-sha256.json)、[源码/库来源](hardware-rtx3060/evidence/manifest.json)已核对。检查包括脚本语法、插桩release构建、实卡回放、边界时序/闭合、物理工作量与记录轨迹对照；未新增永久测试。生产crates/Godot没有改动，本地生产release库重建后SHA恢复为69deadd8…f32ec6。

结果和来源取回核对后停止实例58263433，平台确认[stopped/exited](hardware-rtx3060/final-stop-status.json)。本轮小时费分量保守估计约$0.0344，连同上一轮估计仍在既有$1运行预算内；不含网络和保留磁盘费用，不是平台账单，见[费用估计](hardware-rtx3060/cost-estimate.json)。保留磁盘，未销毁。没有自动提交推送或开始下一轮优化。
