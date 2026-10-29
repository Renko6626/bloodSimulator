# 细胞动态网格、LBM合批与HUD降频

2026-10-30。用户确认[推荐设计](../../superpowers/specs/2026-10-30-cell-render-lbm-batching-design.md)，在当前feat/godot-static-background分支实施；用户随后要求阶段性提交推送，本次纳入归档，优化实施未启动云实例。基线d0dfa07，已有研究与脏文档保留。步骤见[计划](../../superpowers/plans/2026-10-30-cell-render-lbm-batching.md)。

## 实现

- Godot每个可见细胞复用一个ArrayMesh，外层与0.48内层放在同一surface，通过顶点色保留原配色。每帧对真实轮廓原生剖分一次，两层共用拓扑；更新顶点，仅剖分改变时更新索引，节点数/颜色改变时重建该槽位。已核对2D顶点stride8、小网格索引stride2及实际GL缓冲更新。
- 保留既有相机/AABB剔除，先剔除再准备网格；槽位按snapshot顺序复用，数量减少释放多余引用，不保留远场身份。保留原抗锯齿轮廓、debug与机器人画法。headless或禁用render-loop时不构造动态绘图资源。没有修改Rust snapshot接口或默认软体形状。
- LbmWindow增加相同StepInput的advance_batch；一批上传一次Params，用一个encoder依次记录原collide/openings并交替population，末尾submit一次。StreamingWindow原有批次接口接入此路径。两个机械批次、14格子步、边界、均值控制和三次正常CPU读回均保留。
- HUD只把统计/流量文本降到墙钟100ms一次。暂停和单步控件在限流前刷新，不引入新UI或调度体系。

## 本地渲染结果

Godot4.7.1、GL compatibility、Xvfb/llvmpipe、1280×800，默认解析流世界，无键鼠或UI交互。两侧使用完全相同的基线release桥接库69deadd8…f32ec6和10Hz HUD，只改变细胞绘图。第120步截图；180预热后测600帧，最终均到780步、相同玩家位置。

| 指标 | 原画法 | 复用Mesh |
|---|---:|---:|
| World绘图脚本均值 ms | 1.496 | 1.348 |
| World绘图脚本p95 ms | 1.826 | 1.618 |
| 整场draw calls均值 | 147.158 | 125.927 |
| 软件渲染帧间隔均值 ms | 24.511 | 24.931 |
| 软件渲染帧间隔p95 ms | 29.094 | 29.178 |

绘图脚本均值下降约9.9%，draw calls下降约14.4%，但本轮**没有测出端到端提速**。这是一轮软件渲染顺序对照，不是RTX FPS成绩，不能用提交数量降幅推算整帧提速。保留原AA轮廓的方案收益较温和，没有在本轮自行扩大为自定义描边/shader或全细胞合批。

已查看[原画法截图](local-checks/baseline.png)和[Mesh截图](local-checks/candidate.png)；排除右侧HUD，左侧990×800共792000像素完全一致。HUD按真实墙钟更新，截图同模拟步时可能显示不同采样时刻的统计，不作为轨迹差异。见[像素检查](local-checks/visual-comparison.json)和[600帧汇总](local-checks/render-summary.json)。

外观等价只覆盖上述默认场景。原命令顺序为外层→轮廓→内层，当前双层Mesh→原轮廓；极薄/深凹形状若内层触及外轮廓，可能有边缘层叠差别。独立审阅将其列为Minor，符合已批准路线，本轮不扩展新的描边机制。凹形填充、索引及顶点变化通过真实GL几何用例；它不证明所有压扁状态的像素等价。实际reset/进出视野的代码路径已审查，未进行自动UI交互验收。

## LBM工作量与正确性

扩展已有batched_advance_matches_individual_steps，用非零且跨批改变的加速度、4/3奇偶步，比较完整宏观场。旧实现先失败于5次submit≠2；合批后通过。每批一次格子求解submit，另加原有一次场读回。不是仅验证mock或常量。

现有Coarse runtime用例验证8格子步、21dispatch、8submit、3读回，以及帧末控制/缓存、暂停和时间规则。另用现有lbm_gameplay_probe跑Fine移动验证场：120预热＋300测量步，72详细细胞，实际移窗1次，机械/流体运行守卫通过。

| Fine实际工作 | 帧数 | 格子步 | dispatch | submit | 读回 |
|---|---:|---:|---:|---:|---:|
| 普通帧 | 419 | 14 | 33 | **8** | 3 |
| 移窗帧 | 1 | 14 | 34 | 10 | 4 |

此前普通帧为20submit，本轮为8，减少60%；shader工作量和正常读回量没有减少。这不是LBM耗时下降60%的证据。移窗仍有原来的prepare及额外读回成本，未声称修复移窗尖峰。软件probe是功能验证，不是硬件性能验收。见[完整CSV](local-checks/fine-functional/move.csv)、[状态](local-checks/fine-functional/status.txt)和[工作量核对](local-checks/fine-summary.json)。

## 检查与复现

以下命令实际通过，完整日志在local-checks：

```bash
bash scripts/check.sh
cargo check --locked -p godot_bridge --features lbm-gpu
cargo test --locked -p simulation --features lbm-gpu fluid::streaming_window::tests -- --ignored --nocapture
cargo test --locked -p simulation --features lbm-gpu lbm_runtime_tests -- --ignored --nocapture
cargo run --locked --release -p simulation --features lbm-gpu --example lbm_gameplay_probe -- \
  --frames 300 --warmup 120 --case move --local-cells --functional --allow-software --output <result-dir>
cargo build --locked -p godot_bridge --features lbm-gpu
cargo build --locked --release -p godot_bridge --features lbm-gpu
```

现有默认检查包括fmt、workspace check、simulation测试、桥接构建、cached editor import及world/lab各180帧headless回放。相关软件GPU窗口/耦合用例均通过；新增短GL脚本直接读取实际surface缓冲，验证凹形面积、顶点/索引更新、节点数和空输入，不引入测试框架。

```bash
DISPLAY=<X-display> LIBGL_ALWAYS_SOFTWARE=1 godot --path godot --audio-driver Dummy \
  --script res://tests/world_cell_mesh_test.gd
```

此脚本需要真实GL后端，不能用dummy/headless的旧缓存数组证明GPU region更新。渲染对照入口[render-check.gd](local-checks/render-check.gd)，对应插桩World源码和库哈希保存于local-checks；[manifest](local-checks/manifest.json)区分渲染旧库与新增LBM合批release库5c496a4f…ca6576。

独立只读审阅未发现Critical/Important；[审阅记录](review.md)保留结论与未覆盖项。独立基线的第一次空缓存editor导入退出134，缓存重试0；与此前已知异常一致，根因仍未解决，不能将重试写作修复。主项目本轮scripts/check.sh及GL回放退出0。

尚未做RTX3060端到端复测、全路径画面覆盖或人工键鼠手感验收。没有再次启动Vast实例；此前实例仍按上一轮收尾记录停止。没有自动改GPU归约、机械步数、读回依赖或渲染后端。
