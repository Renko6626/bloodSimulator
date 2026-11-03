# 心口解析边界对齐实施计划

> **For agentic workers:** Use superpowers:executing-plans inline. 用户已确认所述短设计并要求执行；沿用现有独立工作树，不重复询问执行方式。

**Goal:** 烘焙时在实际心脏交接四口施加解析速度，导出同面边界迹及插值支撑，通过原背景校验后接入无细胞运行入口。

**Architecture:** 共用原LBM平均逻辑和set_opening_profiles；新增边界对齐导出，内部只采样真实格心数据。BFLOW001、图驱动、求解器及默认路径保持。

**Tech Stack:** 既有Rust/wgpu/Godot，无新依赖。

**Spec:** `docs/superpowers/specs/2026-11-03-heart-boundary-alignment-design.md`

## Global Constraints

- 真地图/四腔不改，16块、5×5现场；所有实时LBM许可关闭。
- y656四口、候选域[464,656]..[656,848]，h=.5/384²；仅CPU真实检查后固定参数。
- 同一GraphNetwork Q48、黏度.05、H1；不改容差，不以解析场覆盖内侧速度。
- 一次有界48000步/60样本心脏作业，墙钟1200秒；失败停留证据，不自动重跑/转云。
- 无细胞阶段；原无流预览、默认世界和旧实验保持；UI不自动键鼠交互。
- 测试以AGENTS.md范围为准：仅新增两个关键覆盖及最终入口/reset真实测试，不为文案/配置建立测试层。

## Review Focus

- 连续解析与离散归一差别由原分辨率容差覆盖，不能声称逐位相同。
- 显式剖面必须在第一次推进之前应用；旧bake_mean默认语义保持。
- 导出只在已知域边界和外排使用解析值，内侧一律真实场采样。
- f32弧长重建点需插值支撑；四口恰好距节点2r时仍通过原检查，不改阈值。
- 所有边界和内侧采样都要记录；匹配边界不等同于内部稳定或FPS验收。

### Task 1: 显式剖面、对齐导出、CPU预检

**Files:** Modify `crates/fluid-gpu/src/lbm/bake.rs`、`crates/simulation/src/offline_lbm.rs`、`crates/simulation/examples/organ_heart_bake.rs`；Create `crates/simulation/src/map/organ_flow.rs`，在map.rs导出小型模块。

**Interfaces:** 保留`bake_mean(...)`；增加`bake_mean_with_profiles(domain, config, initial, geometry, profiles: Option<&[Vec<f32>]>, schedule)`。新增`to_boundary_aligned_field(map, flow, baked, geometry_revision, reference_pressure, max_scale, ports)`。地图元数据提供domain常量、ports和政策装配，无GPU依赖。

- [ ] 在已有offline_lbm测试模块添加真实导出测试：独立给定直管Q，检查边界与域外支撑的数值、内侧非解析输入仍被保留、固体为零；先红后绿。
- [ ] 实现上述导出：同h节点对齐网格向外一排，内部原采样，边界/外排仅声明端口支撑使用同一解析场，其他外部流体拒绝。
- [ ] 给既有平均作业加可选显式profiles，首次advance前调用原setter；旧入口包一层None，不复制循环。
- [ ] 在原GPU测试位置加一个ignored小用例，实际调用新入口，施加区别于默认抛物线的平坦剖面并检查真实烘焙边界输出；显式软件运行一次验证。
- [ ] 更新心脏例子与共用元数据：384²/y656/四口/新标签，向新平均入口传入raster.openings.normal_velocity；保存原场及边界对齐场。
- [ ] CPU预检：原实际几何、四面源边界、方域开口/分量、原端口余量；不创建GPU。

验证：新CPU测试、已有受影响测试；显式ignored软件小用例；`organ_heart_bake --preflight`退出0及生产端口几何检查。CPU预检未通过不启动完整心脏作业。

### Task 2: 一次真实有界作业及原校验

**Files:** `docs/research/2026-11-03-organ-heart-flow/attempt-03/`，必要的例子诊断输出。

**Interfaces:** 消费Task1例子与元数据，输出原场、对齐场、来源、进度和原策略导入结果。

- [ ] 记录源码/二进制哈希和授权，按固定60+60模拟秒/60样本/48000步、最长1200秒运行一次本地软件作业。
- [ ] 真实写入/读回后通过原`try_new_policy_world`；保留首次失败的具体原因，不放宽检查。
- [ ] 记录四口边界与内侧采样、密度与半均值差、实际图像。不以端口匹配证明稳定；失败止于此，不启动第二次完整作业。

### Task 3: CPU运行入口与集中验收（依赖Task2通过）

**Files:** `assets/flow/organ-heart.bfld`、runtime/policy.rs、godot_bridge/src/lib.rs、godot/world.gd、world_hud.gd、研究记录及STATUS。

**Interfaces:** `Simulation::try_new_organ_world(Config)`、bridge `use_organ_world() -> bool`、`--organ-world`。

- [ ] 复用原已确认入口设计，嵌入验收资产，设置参考驱动和真实压力上限；红白细胞为0，reset保留场景/压力/暂停。
- [ ] 一个真实生产构造/reset测试覆盖关键新行为；保留所有旧入口语义，不重复旧端口坏数据测试。
- [ ] 接入flag互斥、HUD背景阶段及压力范围，Godot只短加载与截图。
- [ ] 集中fmt、默认/feature simulation测试、fluid-gpu相关测试、all-target feature编译；必要有限跨口回放；只对新变化补检查。
- [ ] 更新阶段记录/manifest/STATUS并作整分支只读审阅，保存本地提交；不自动合并/推送。

## 执行记录

在现有`feat/organ-heart-flow`/宿主管理`7c02`工作树继续，基线579e0b4。旧计划的未完成资产及入口阶段由本计划接续；先前失败资产和原始证据保留。
