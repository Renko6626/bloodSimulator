# 当前状态与待办

更新：2026-10-26。当前开发分支为 `feat/engine-foundation`。本文件维护最新状态和任务；日期报告保留当时的实验、决策与验证证据，不再作为当前任务清单。

## 正式原型

Rust 模拟内核、Rapier 软体与接触、Godot 表现层已经接通。默认世界包含 11 个节点、12 条血管段、心脏和两个器官样板，72 个每个 12 节点的红细胞，以及自由移动机器人、跟随镜头和调试面板。

正式血流仍是“血管阻力网络 → 各段流量 → 局部解析速度 → 细胞”的单向驱动，细胞尚不反作用于正式流场。Stokes、LBM 与 Rayon 改动均在独立研究目录。战斗、治疗、区域激活、远处指标演化尚未实现。

## 本阶段已完成

- [x] 对比有限阻力 Stokes 与 LBM，并验证流体点力向 Rapier 的反冲量回传。
- [x] 在既有 Stokes 求解器中实验 Rayon；进一步复用缓冲、合并任务、减少线程池进入与整场复制。64²/16 细胞四线程整步均值 4.61 ms、p95 6.52 ms，仍未稳定低于 5 ms。
- [x] 同域、同细胞大小扩大到 64 个细胞，复现无欠松弛方案第 16 帧失稳；冻结几何隔离反馈问题，自洽求解完成对照，但整步均值约 40.92 ms。
- [x] 实现点力欠松弛 ω=0.5/0.3，各完成两轮 64 细胞、360 帧轨迹；16 细胞另完成一轮，ω=1 负例复现旧失败。
- [x] 保存源码、锁文件、原始数据、复现脚本、图表、源文件哈希及独立审计，清理 README 的历史结果堆叠和旧存档中过时待办。

## 当前结论

**继续实验的首选是有限阻力 Stokes + 点力欠松弛 ω=0.5，沿用现有 Rayon 四线程实现。** 这是研究候选，尚未决定替换正式后端。

64²/64 细胞下，ω=0.5 整步均值两轮为 9.87 / 10.54 ms，p95 为 12.85 / 16.95 ms；ω=0.3 均值约 10.55–10.62 ms。点力计算与混合阶段约 4 微秒/帧，正常每个机械子步仍只做一次完整流场求解。流体约 5.6–6.3 ms、Rapier 约 4.1–4.4 ms 是主要成本，5 ms 整体目标尚未达到。共享主机存在明显长尾，不构成目标设备性能保证。

ω=0.5 与自洽参照的最大细胞中心差约为直径的 0.051%，但瞬时力律仍有残差和滞后；六秒低速轨迹不能证明长期稳定或普遍物理精度。模型只有每细胞四个内部阻力点，允许透流，尚未实现真实膜排液或严格堵塞。

镜头规模按用户描述是大几十至数百细胞，近景可为几十个。详细模拟数量、视野物理尺寸和预算仍需结合玩法确定；不可把此前小直径 packed-LBM 容量测量当成当前同大小 64 细胞的等精度对照。

## 待办与优先级

以下是待开展工作，本次存档不自动启动新实验或实施路线变更。

1. **先定位当前候选的热路径。** 在 ω=0.5、64 细胞场景分别拆解流场 FFT/模式求解与 Rapier 密集接触成本；保留不含细分埋点的性能对照。根据证据选择局部优化，避免并行化仅几微秒的点力混合循环。
2. **补齐有针对性的稳定性与响应验证。** 在当前路线下选择更长时间、不同密度/布局或驱动反转的少量场景，继续记录力律残差、冲量、面积、响应滞后与自洽参照差异；规模与时长按具体问题控制，不默认穷举。
3. **明确游戏内负载与预算。** 确定典型近景中必须真实推挤的细胞数，以及大视野中哪些区域可以使用指标。已确认方向是近处详细、远处区域指标，相邻活跃区域共用 Rapier 世界；尚未实施 LOD。
4. **正式接入前单独形成方案。** 局部流场窗口与血管网络的单位、开口边界、初始化/切换、不规则血管几何尚待研究。当前周期直管求解器不能直接代表游戏地图；接口和模块边界须先确认。
5. **玩法批次仍待推进。** 战斗与治疗、区域衔接、远处演化继续留在后续计划，不因流体实验扩展当前产品范围。

RustFFT 等新依赖、GPU/新求解器、联合隐式积分、严格膜封堵、复杂自适应加速或更细 LOD 层级均未获实施确认。若已有路线无法满足目标，应先展示成本与取舍再选择。

## 文档与存档范围

- [最新欠松弛报告](research/2026-10-26-stokes-relaxation.md)与[完整研究索引](research/README.md)。
- [世界实施记录](superpowers/plans/2026-10-22-world-small-map.md)、[正式原型性能诊断](superpowers/plans/2026-10-22-performance-audit.md)。
- [整体架构](superpowers/specs/2026-10-21-world-architecture-design.md)与[世界小地图设计](superpowers/specs/2026-10-22-world-small-map-design.md)。

此次提交保存热路径优化、64 细胞失败与自洽对照、稳定耦合调研、欠松弛实验及本次文档整理；此前有限阻力与 Rayon 阶段提交为 `f1313f9`。保留旧基线和原始输出，避免破坏复现路径与哈希。

## 验证记录

提交整理只改文档与导航，不改已计时源码或原始 CSV。提交前实际执行并通过：

```bash
python3 docs/research/experiments/2026-10-26-stokes-hotpaths/analyze.py
python3 docs/research/experiments/2026-10-26-stokes-64-cells/analyze.py
python3 docs/research/experiments/2026-10-26-stokes-relaxation/analyze.py
python3 docs/research/experiments/2026-10-26-stokes-relaxation/review/recompute.py
cargo fmt --check --manifest-path docs/research/experiments/2026-10-26-stokes-relaxation/solver/Cargo.toml
rustfmt --check --config skip_children=true docs/research/experiments/2026-10-26-stokes-relaxation/dynamics/src/main.rs
cargo check --offline --locked --manifest-path docs/research/experiments/2026-10-26-stokes-relaxation/dynamics/Cargo.toml --target-dir /tmp/sunyunbo/blood-stokes-rayon-target
```

三组分析及独立复算均符合归档结果，ω=1 的失败保留为负例；16 份本次新增或整理文档的本地链接检查通过。未重新运行全部历史性能实验、正式游戏或 UI 交互，也未新增永久测试。

暂存区全量 `git diff --cached --check` 仅报告三个生成 SVG 的尾随空白，保留生成文件原样；源码、锁文件和文档的空白检查通过。
