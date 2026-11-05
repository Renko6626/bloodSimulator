# 基础细菌行为模板

2026-11-05。实现分支 `feat/bacteria-behavior`，基于 `main` 的 `0bd496a`。用户已确认设计并要求开工；本阶段不自动合并或推送。

## 行为与配置

细菌主动寻找附近可感知的红细胞，追近并通过真实轮廓接触造成伤害。两种现有细菌各自配置受击反应：

- `Retaliate`：反击直接攻击自己的机器人或免疫细胞；攻击者不可感知后清除自卫记录，恢复寻找红细胞。未受攻击时不会主动追击机器人。
- `Flee`：停止接触攻击，按有限转向与局部避墙向威胁相反方向游动；失去感知后只使用最后逃离方向，默认持续 0.8 模拟秒，再恢复寻找红细胞。

两种细菌默认都保留反击。以下配置在构造模拟时传入，不需要新增 UI 或命令行开关：

```rust
use simulation::{Config, Simulation, ThreatResponse};

let mut config = Config::default();
config.bacteria.motile.response = ThreatResponse::Flee;
config.bacteria.motile.flee_duration = 0.8;
config.bacteria.encapsulated.response = ThreatResponse::Retaliate;
let sim = Simulation::new_combat_preview(config);
```

reset 保留配置并重新初始化行为。休眠期间保存身份、生命与行为，冻结逃离余时；恢复后重新检查感知并清空接触准备。细菌仍使用原有速度、攻击数值、身体与外观，低速荚膜细菌并不会因选择逃离而突然加速。

## 实现位置

- `crates/simulation/src/pathogen.rs`：细菌独立的目标选择、自卫与逃离决策，以及按种类配置。
- `crates/simulation/src/behavior.rs`：行为枚举、运行期提供的感知事实、共用运动/接触状态及有限转向；不保存免疫呼叫或逃离专属计时。
- `white_cell.rs`：免疫专属的记忆、调查、追捕、呼叫和入场状态。
- `runtime/immunity.rs`：准备局部感知与受击事实，分发决策，复用身体驱动、遮挡、真实接触及统一伤害结算。

快照和 Godot bridge 字段保持现状。逃离时不输出攻击目标、机器人敌对标记或攻击准备弧；运动本身表现逃离。普通世界种群不变，细菌仍在战斗预览中观察。

## 检查记录

新增四项真实模拟测试覆盖机器人反击/遗忘、两种细菌实际逃离、遮挡后的方向与余时及恢复捕猎、休眠冻结与恢复。既有预览 reset 用例补充按种类配置保留的断言。测试调用生产决策、物理或真实轮廓接触结算，没有 mock 或源码字符串断言。

实现前先观察机器人反击用例失败（机器人生命仍为100，预期92）；逃离实现前观察运动与禁止攻击目标断言失败，休眠用例的逃离速度断言也失败。接入后相关16项战斗检查通过。集中检查结果如下，均退出 0：

| 实际命令 | 结果与证据 |
| --- | --- |
| `bash scripts/check.sh` | fmt、workspace 编译、64 单元＋5 集成、桥构建、首次 Godot 导入、world/lab 各180帧加载通过；[日志](logs/check.log) |
| `cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu` | 可选路径全目标编译通过；[日志](logs/feature-check.log)，未启动GPU模拟 |
| `godot --headless --path godot --fixed-fps 60 --quit-after 10 -- --combat-preview` | 战斗预览10帧加载通过，无脚本错误；[日志](logs/combat-load.log) |
| `git diff --check` / `git diff --cached --check` | 无空白错误 |

[独立代码审阅](review.md)未发现需修正的问题。未进行自动键鼠、UI流程或性能测试。

## 试玩入口与边界

在本分支工作树运行：

```bash
bash scripts/run.sh -- --combat-preview
```

默认展示反击型行为；逃离型通过上面的 Rust 配置启用。本轮不新增配置面板、外观动画、繁殖、感染、种群投放或性能实验。

自动检查不等于人工手感验收。逃离是否容易辨认、追逐节奏是否合适、全图拥堵和新战斗负载 FPS 仍待人工体验；本轮不进行自动键鼠/UI交互或云实验。

## 相关文档

- [设计](../../superpowers/specs/2026-11-05-bacteria-behavior-template-design.md)
- [实施计划](../../superpowers/plans/2026-11-05-bacteria-behavior-template.md)
