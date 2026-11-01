# 执行决定与验收边界

本阶段按用户“开干”确认的完整分级方案推进，沿用已有模型及预览入口。以下保留本阶段执行账本中的全部决定及代价，按发生顺序排列。

1. Ruling: follow explicit “开干” approval for full geometry route without another plan approval round — approved model is unchanged; cost if misread is reversible local geometry work.

2. Ruling: extend existing heart-preview map and keep its zero-state factory/HUD — avoids a new scene/model boundary; cost is larger explorable preview, historical small-layout evidence remains versioned.

3. Ruling: use private authored repetition helpers for this map only, not a generic generation framework — repeated branching is implementation detail; cost is less reuse by future maps.

4. Ruling: consolidate full checks in Task2, with meaningful focused map/flow tests in Task1 — user verification rules override blanket TDD; cost is no exhaustive arbitrary-layout coverage.

5. Ruling: preserve host worktree and preceding branch refs, new local branch includes prior offline/heart work — cost is eventual integration includes those stages; no push/merge now.

6. Ruling: run one long real CPU replay in release with unchanged speed8/dt/substeps/collision guards — avoids debug verification overhead; cost: timing cannot be compared with debug and is not a performance claim.

7. Ruling: after SVG-title-only numeric annotation fix, rerun only affected fmt/all-target/export, preserve original movement replay and exact old source — user forbids restarting complete validation for local output edits; cost: final diagnostic source differs from replay binary only in documented nonmovement code.

8. Ruling: rerun the affected full input probe once after final-return fix, rather than infer global clearance/peak from the failed run — original global min masked earlier minima and final peak assertion did not execute; cost another~20min CPU replay, while all unrelated suites remain retained.

## 已完成与保留边界

- 98节点、224管段、两套串联分级模块、128条独立微血管，实际地图外框1424×2719单位。仍使用16单位块、5×5现场和零流/零细胞预览。
- 默认黏度0.05的生产图求解：参考驱动30.277714，对应Q48；这是等效图模型数值，不是生理压力或已启用的游戏血流。
- 第一次全图回放到达210点，但最终净空0.85198593未达1.0；原日志保留。最后回房路线遗漏真实弯点，已补点并新增快速连续路线段SDF检查，先复现失败再通过。
- 修正后完整回放211点、87771步、实际距离11684.384；两心室跨度83.985/89.943，最低实际中心净空1.273，峰值25块，逐帧零状态及最终守卫均通过。
- 原默认/feature套件和项目加载检查通过；局部诊断输出、类型和路线修正后只补验相关检查，没有反复重启完整套件。详见README、provenance和原始日志。
- 尚未验收人工操作手感、任意路线、心腔烘焙/场采样、细胞分布运输、动态瓣膜或实卡性能；没有云实例或新性能实验。
- 本地分支feat/heart-vessel-layout基于已完成但未合并的离线LBM/心腔预览阶段，最终集成会同时纳入这些提交。本轮保留宿主管理工作树，不push/merge。
