# FD 梯度正则化开口实验

2026-10-28，分支 `experiment/lbm-fd-boundary`，基线 `21ee36a`。仅本机 **llvmpipe，software=true**；没有云端实验、生产数值改动或默认配置变化。

**FD 在本轮条带诊断中消除了已知出口增长模；四个保持原低黏度参数和给定开口速度的无机械案例均完成160帧。它是研究候选，尚未通过首发验收。** 斜向开口缺少所需 stencil；分叉内域速度超过 `.05` lattice 参考值，密度/质量精度及长期稳定性没有验收。

## 方法和证据

[调研来源](sources/literature.md)、[先行计划及具体公式](../../superpowers/plans/2026-10-28-lbm-fd-boundary.md)、[此前讨论快照](sources/boundary-next-steps-context.md)。计划先写后实施，主代理已检查。快照“未启动”描述此前状态。

重建九个开口分布，非平衡应力来自 streaming 后的内部速度梯度及给定边界速度。主密度闭合保留 `(A+2B-Fn/2)/(1-un)`；Latt BC4也采用这种闭合，本轮检验其与FD应力的组合。MRT迹/偏应力分别使用 `se=1.1` / 原 `snu=1/tau`，Guo含半步力、应力源项及shifted populations修正；这是研究中的前导阶适配，文献没有证明本参数下完整MRT边界稳定。保留所有原碰撞、nu、dt及离散目标流量，没有clamp、CBC、NEQ、固定密度出口或质量反馈。内部梯度仍影响开口，未重置成解析平衡场。

32×32条带、U=.022、y周期，完整288×288线性算子的全部17个独立ky：

| 配置 | 原重建最大谱半径 | FD非零ky最大谱半径 | FD含ky=0 |
| --- | ---: | ---: | ---: |
| Fine，tau=.50285715 | 1.0643227814 | .9970429480 | 约1 |
| Coarse，tau=.50125 | 1.0690172917 | .9985316323 | 约1 |

`ky=0`均匀密度向量的直接残差≤3.4e-16，属于简化域中性模，**不等于实域质量守恒**。预声明rho_1外推只跑谱作敏感性对照，没有扩展实域矩阵或改默认。

新编译WGSL注入原k13模，Fine100步：baseline投影从1.0000068e-6增至3.1948257e-4，实测约1.059362/步；FD降至1.0452337e-8，总population L2从1.2154428e-5降至2.7750608e-6。总范数后段略回升，不能仅凭旧模投影衰减声称整个算子稳定。Coarse也跑了同种子100步FD，见日志；无性能结论。

## 无机械实域结果

直管复用旧诊断参数，分叉复用硬件probe/独立调查地图及生产GraphNetwork/rasterizer/opening_profiles。无Rapier、细胞、机器人或迁移。Fine192²、h=.25、dt=.0011904762；Coarse96²、h=.5、dt=.0020833334；nu=.05。每个lattice步检查正密度和rho/u有限性。**表中极值仅来自帧末采样**。

| 案例 | 帧/步 | rho范围 | 最大lattice速度 | 质量相对变化范围 | 末帧质量变化 |
| --- | ---: | --- | ---: | --- | ---: |
| 直管Fine | 160/2240 | [.9555325,1.0451314] | .04292709 | [-.02423%,+.10031%] | +.06716% |
| 直管Coarse | 160/1280 | [.9750874,1.0243366] | .03382204 | [-.01897%,+.06012%] | +.05930% |
| 分叉Fine | 160/2240 | [.94133455,1.0478976] | .09118936 | [-.03453%,+.14121%] | -.01615% |
| 分叉Coarse | 160/1280 | [.9434584,1.0669252] | .078689836 | [-.02942%,+.13659%] | +.08686% |

四例帧末逐格边界速度最大误差≤2.3841858e-6世界单位/时间，各开口积分通量误差≤约6e-6世界单位²/时间。四帧世界加速度[10,-7]检查的边界速度误差≤1.4305115e-6，仅支持Guo半步力下速度/一阶矩一致性，不代表完整二阶应力或耦合精度。质量用shifted密度增量及f64求和；未设精度门限，漂移观测不能称为质量守恒通过。

原出口没有再出现非正密度/非有限坏格，但rho波动仍明显。Fine直管末帧最小rho仍在右出口[191,95]；Fine分叉末帧最大速度在内部结点附近[118,107]。位置和每帧读数均留在日志。

45°直管 `y=x+12` 可由当前rasterizer描述，但Fine左端cell32、Coarse左端cell16的第1层内法向已遇墙，研究模块明确返回 `FD stencil unsupported`，未推进该域。没有修改几何算法、把固体当流体或增加stencil回退。该诊断最初误用无回路GraphNetwork触发断言；改用给定GraphFlow后得到上述有效检查，失败记录单独保留。仓库未发现既有45°流体运行fixture，因此这里只新增最小几何诊断数据。

## 复现、来源和检查

在实验worktree根目录：

```sh
docs/research/2026-10-28-lbm-fd-boundary/run.sh
```

最终归档命令退出0；它重新编译研究源码和内嵌WGSL，只复用外部依赖rlib，未使用旧数值binary。默认依赖在主checkout `target/release/deps`，可用 `LBM_DEPS` 指定等价缓存。线程限制BLAS/OMP=1、llvmpipe=2，实域最多同时两例。`bin/`保留本机但Git忽略。

- [源码](src/)、[build.sh](build.sh)、[run.sh](run.sh)、[汇总计算](analyze.py)。pad模式3=baseline条带，7=FD条带，8=FD实域；9/10=rho_1对照（10未运行）。
- [环境](results/environment.txt)、[源码/binary清单](results/build-manifest.sha256)、[依赖清单](results/dependency-manifest.sha256)、[结果清单](results/results-manifest.sha256)、[检查输出](results/checks.log)。实验基线与硬件源 `7727c60` 的 `crates` 差异为空。
- [完整谱](results/spectrum.json)、[汇总](results/summary.json)、[baseline WGSL](results/eigen-baseline.log)、[Fine FD](results/eigen-fd.log)、[Coarse FD](results/eigen-fd-coarse.log)。
- [直管Fine](results/straight-fine.log)、[直管Coarse](results/straight-coarse.log)、[分叉Fine](results/fork-fine.log)、[分叉Coarse](results/fork-coarse.log)、[强迫一阶矩](results/forced-moments.log)、[斜管stencil](results/diagonal-stencil.log)。

检查仅包括研究构建/诊断、汇总完整性、格式、shell语法、whitespace及清单；未新增永久测试、运行无关全量测试或浏览器交互。格式化后曾为来源统一重复一轮既定案例，这不是新增验收要求；主代理指出后停止数值运行。

未验证：斜向端格闭合、完整墙面/分叉谱、密度/速度精度、长期质量趋势、机械耦合、迁移、实卡稳定和性能。下一步需先决定是否继续研究wall-aware stencil及确定精度门限，之后再安排相应验证。本轮不自动开展，也不合并生产分支。

后续已授权的有限邻域LSQ-FD候选及选定斜管/分叉结果见[独立候选记录](lsq-candidate/README.md)。本页保留原FD方案和原始结论。
