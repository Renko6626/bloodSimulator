# 帧末平均密度控制候选

2026-10-29，基线 `ba279e8`。RTX3060、`software=false`，同设备同初态 `fork_a Fine` baseline/control各1000帧；保持MRT+Guo、LSQ-FD、原nu/dt和给定速度/70:30流量。只有control每帧末施加一个空间均匀平衡分布差。

**1000帧内，平均密度按设计保持在1，局部矩观测通过，选定场差较小。补偿总账仍显示持续净去除质量；这不等于边界已经自动守恒，也不证明物理误差已解决。**

## 修正及即时观测

每14格子步后CPU用raw shifted populations、f64计算 `delta=meanrho−1`。GPU对每个有效流体格施加

```text
gnew = g + feq(rho−delta,u)−feq(rho,u)
     = g−delta*w*(1+3cu+4.5cu²−1.5u²)
u = (j+F/2)/rho，F保持不变
```

理想代数上rho统一减delta、原物理u及相对新平衡分布的非平衡残差保持，瞬时rho空间差保持。保留MRT碰撞，未采用BGK、直接f0修正或全部分布比例缩放。借鉴[OpenLB TR3](https://www.openlb.net/wp-content/uploads/2011/12/olb-tr3.pdf)的均值偏移思路，具体调度/碰撞与该来源不同，不声称其验证了本候选。

下表是1000次实际GPU修正前后，独立raw矩公式测得的最大变化；预声明门限均为5e-7，未作运行后放宽：

| 观测 | 最大值 |
| --- | ---: |
| 修正后平均rho与1的偏差 | 2.11975e-10 |
| 各格Δrho与统一−delta的偏差 | 2.11867e-9 |
| lattice速度变化范数 | 2.03167e-9 |
| 非平衡应力分量变化 | 1.58223e-9 |

非零Guo单次moment-only检查从rho=1.02开始，显式GPU force=[.02,−.01]，实际delta≈.02；u变化7.15e-10、Guo修正非平衡应力变化5.35e-10。应力定义为 `Σccf−rho*uu−cs²rho*I+(uF+Fu)/2`。该检查不含推进步骤，不代表受力/机械耦合精度验收。

## 成对演化与补偿总账

每窗口完成14000格子步，每步检查正密度和rho/u有限性，无坏格。baseline末平均rho=1.0008034973，control=1.000000000042。baseline末质量增长5.737774852个格子和单位；control的solver累计增长5.737478972、实际净去除5.737478008，末质量较初态增加9.64e-7格子和单位（相对变化1.35e-10），补偿预算残差0。乘h²D=.0625后，实际净去除量为.3585923755。

累计去除44.681971、补入38.944493，净去除5.737478；本例最大单帧|delta|约4.10e-5。**每帧控制同时去掉了全域平均密度的周期性起伏和慢漂移。** 去均值rho比较本来排除了这个全局模，因此不能声称全部压力响应不受影响；未来依赖绝对/平均压力的耦合未验收。

下表为matched frame-end相对L2差（%，峰值/末值）；rho在每个比较区域分别去均值，作为压力空间变化的proxy。中心区为[16,32]×[16,32]世界单位内的流体格。

| 区域 | 速度 | 去均值rho | 非平衡应力 |
| --- | ---: | ---: | ---: |
| 全域 | .03459 / .01472 | .11960 / .08955 | .11419 / .08031 |
| 中心 | .05510 / .02032 | .13324 / .09016 | .14088 / .07918 |

预声明比较门限为速度5%、去均值rho5%、应力10%，全程未超界。baseline并非物理解真解；瞬时非平衡残差保持也不代表演化应力不变。绝对RMS/最大场差及峰值帧号保存在[分析数据](results/hardware-analysis.json)。边界世界速度误差baseline/control最大6.30e-7/7.21e-7，体积流量误差1.74e-6/1.87e-6。两者最大lattice速度约.06831，仍高于原.05参考值。

## 来源、检查与限制

[先行计划](../../superpowers/plans/2026-10-29-lbm-mean-density.md)、[源码](src/)、[编译](build.sh)、[GPU命令](hardware-rtx3060-20261029/run-hardware.sh)、[原始结果与三份末帧raw快照](hardware-rtx3060-20261029/results/)、[汇总计算](analyze.py)、[冻结SHA](results/frozen-source-binary.sha256)、[检查输出](results/checks.log)。运行只需binary与旧 `fork_a-fine.geom.weights`，正式参数为 `LBMDIAG=12 LSQ_WEIGHTS=./fork_a-fine.geom.weights ./mean_density --frames 1000 --output results/paired`，不带软件适配器许可。

本机只跑2帧paired smoke及既定force检查，随后源码/binary冻结；正式GPU成对实验仅一次，没有额外长程/角度矩阵。原MRT/stream/LSQ数值WGSL文本未改，单独追加normalization pass。格式/语法、来源和19项GPU结果SHA通过。实例58133143已由主代理确认stopped/exited，16GB磁盘保留。

这是当前固定几何、无机械案例的短程候选。更长时间、其他几何、绝对压力/全局压力波、迁移及耦合均未验收；未推广生产、合并或推送，也不自动追加实验。
