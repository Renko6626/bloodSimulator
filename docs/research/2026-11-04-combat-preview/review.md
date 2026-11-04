# 第一批免疫与病原体战斗独立审阅

日期：2026-11-04。基点：`4ff964b`。审阅对象为当前未提交工作树及 `review-package.md` 中全部 tracked / untracked 改动，包含 `entity.rs`、`pathogen.rs`。按 task brief 和已批准设计核对；实现报告只作线索，不把报告中的通过结果当作本次独立执行证据。未修改产品、index 或 HEAD，未派生代理。

## Findings

### Important 1：切换目标后残留的直接攻击者身份绕过既有记忆期限

- 位置：`crates/simulation/src/runtime/immunity.rs:128`（旧 `attacker` 优先于当前目标），`crates/simulation/src/runtime/immunity.rs:148`（细菌放弃目标时也不清理 `attacker`），`crates/simulation/src/white_cell.rs:106`（每个新可见目标都会刷新同一个 `memory`），`crates/simulation/src/white_cell.rs:114`（只有当前目标不可见、到达最后位置或记忆到期时才清除 `attacker`）。
- 触发：机器人攻击中性粒后离开其感知；中性粒遇到可见病原体并切换目标。病原体持续可见时，新的目标不断刷新 `memory`，旧的 `attacker=Robot` 不会失效。即使玩家已脱离超过设计的 2～5 秒且通缉已降为 0，返回感知范围后仍会压过当前有效病原体目标。
- 影响：低通缉追捕和直接受击证据实际可以无限保留；玩家利用地形脱离并等待后，依旧会因很早之前的攻击被重新优先追捕。细菌在失去全部目标时主动清空 `goal`，也会使旧免疫攻击者记录失去清理机会。这与按既有记忆结束追逐及保留当前有效目标的规则不符。
- 证据：在 `/tmp/sunyunbo/combat-review-20261104` 复制当前模拟源码，临时追加一个真实模拟复现，调用生产的玩家攻击、物理帧推进和目标逻辑。中性粒受到实际 50 点玩家伤害；机器人离开 12 秒，期间中性粒保持荚膜细菌目标，通缉降为 0；机器人返回一帧后输出 `wanted=0, robot_hp=100, target=Some(Robot), attacker=Some(Robot)`。断言应继续当前细菌目标失败（实际 `Some(Robot)`，预期 `Some(Cell(12))`）。工作区产品及永久测试未改动。
- 修正范围：在放弃旧来源、切换到其他威胁或既有追逐记忆结束时，使旧 `attacker` 证据失效。沿用现有目标/记忆字段即可，不需要新增独立仇恨计时、列表或恢复机制。

Critical：0。Important：1。Minor：0。

## Spec verdict

**需要修正 Important 1 后通过。** 其余已审阅的首批范围符合设计：

- 显式物种和稳定目标引用；局部距离与固定墙体视线过滤，失联位置使用保存的 `goal`，没有读取休眠身体来追踪目标；接触准备在目标切换、失联和恢复时清零。
- 玩家对红细胞/免疫细胞的实际生命减少计分，病原体受击和生物互战不计玩家通缉；荚膜只减半免疫接触伤害。
- 生物接触读取真实软体轮廓，玩家先结算，再从仍存活生物收集并同批应用接触伤害；允许互杀，来源选择限于批次后存活攻击者并稳定打破同额平局。
- 新物种使用 Hibernate，恢复按本种半径并保留形态、生命和身份；红库存处理仅作用于既有 Aggregate 红细胞，病原体尸体不回流红库存。
- 重型增援只在预览启用；包括休眠和尸体的巨噬槽、免疫总上限、失败消耗呼叫机会，以及普通世界仅中性粒的入口保持一致。
- 预览模式保留 reset 配置；桥与 Godot 同步迁移物种、目标位置、是否针对玩家和准备进度；显示代码包含内部图形、荚膜外轮廓及朝向目标的准备弧，HUD 统计活动身体。

以上为代码路径审阅结论，不把 Godot 加载或编译当作视觉、手感和 FPS 验收。实现者记录的无 DISPLAY / Xvfb、未截图及未人工体验限制仍然有效。

## Quality verdict

**需要一次局部状态清理修正。** Important 1 来自 `attacker` 与当前目标共用记忆但清理条件不同；没有发现需要更换模块边界或增加基础设施的问题。其余实现保持在现有 Cell / runtime / bridge / Godot 分工内，批次为短暂列表，扫描规模与首批场景相称；没有新依赖、A*、化学场、持久队列或额外框架。测试主要复用真实模拟夹具，没有以源码字符串或 mock 断言替代核心行为。

## 本次独立验证

仅针对上述明确疑点运行单一临时复现：

```text
cargo test --locked --manifest-path /tmp/sunyunbo/combat-review-20261104/Cargo.toml --target-dir /data/sunyunbo/www/bloodSimulator/target -p simulation review_old_robot_attacker_survives_other_target_and_expired_wanted -- --nocapture
```

结果：退出 101，1 项复现失败，57 项过滤；失败值见 Important 1。第一次编译因临时副本缺少 `assets/flow/organ-heart.bfld` 未进入测试，补上只读资产链接后得到上述行为结果。没有重跑实现者已通过的整套测试、Godot 加载、GPU / 云实验或 UI 交互。

## Important 1 修正复查及最终结论

日期：2026-11-04。本次仅复查 `review-fix.diff`、实际 `white_cell.rs` / `immunity_tests.rs` 修改，以及原指出的细菌无目标路径，没有重新进行完整审阅。

**Important 1 已解决。** `white_cell.rs:89` 的 `set_target` 现在只保留与新目标一致的直接攻击者记录；选择另一目标或清空目标时同步清除旧 `attacker`。`think` 接收新可见目标、既有记忆到期，以及 `runtime/immunity.rs:148` 的细菌无合法目标路径，均经过这个清理入口。直接受击反应先设置本次来源，再选择相同目标，仍可保留有效的直接攻击者优先级；没有增加计时字段或新的机制。

新增 `runtime/immunity_tests.rs:428` 的回归通过真实玩家攻击、720 帧模拟、通缉自然归零和机器人返回验证原失败场景，同时以实际免疫接触伤害及休眠/恢复验证细菌放弃旧攻击者。预期独立于实现，没有放宽原有断言或改动桥/Godot 接口。

验证证据：主代理交接确认该回归在修正前失败、修正后通过，受影响免疫 12 项、fmt 与 diff 检查通过。本次审阅独立核对实际修正及测试行为，没有重复执行这些检查。

- **Final Spec verdict：通过。** 原 Important 1 关闭，没有剩余具体规格问题。
- **Final Quality verdict：通过。** 修正局部且与现有目标生命周期一致，没有新增状态或扩大模块边界。
- 最终未解决项：Critical 0 / Important 0 / Minor 0。原报告中的视觉截图、人工手感及战斗负载 FPS 未验收范围保持不变；通过结论限于本阶段代码与规格审阅。
