# 细胞近场软体与远场数量：实现与本地验证

2026-10-29，`feat/godot-static-background`。用户确认[具体设计](../../superpowers/specs/2026-10-29-cell-population-window-design.md)后实施。默认仍为全量软体；`--local-cells` 显式开启。没有新依赖，没有启动云实例。随后用户已安排文档、提交与推送，见[阶段交接](../2026-10-29-performance-checkpoint.md)。

## 实现与使用

- 一个 Rapier 世界。runtime 唯一持有活跃软体 handle；近场使用原十二节点材料、接触和机械子步，远场只保存图边连续弧长区间的数量。
- 机器人周围 72×72 激活，完整软体包围盒离开 80×80 保留框才退休。初始中心沿用旧 world/validation 分布；运行中新构造位置限半边长32～36外缘带。
- 远场每0.2模拟秒按 `u=Q/(πR²)`、`ΔN=N×min(1,|u|Δt/L)` 输运。一次更新只读旧库存；节点按下游流量绝对值分配，近场未实体化库存停止抽象输运。
- 移窗按弧长重叠重分配库存。合法空位找到后才扣除一个整数份额；拥挤和分数库存原样留下。退休调用真正的 `remove_soft_body`，不保留远场个体副本。
- snapshot/HUD 明确 Detailed、Unmaterialized、Total。Unmaterialized 包含远场及近场尚未构造的份数，不能另加一次“边界余额”。
- LBM与背景拖曳只遍历活跃列表。群体模式停用按数组索引追踪个体的速度/累计位移字段；空窗口不输出不存在的细胞形状成绩，仍检查机器人和流体。

```bash
bash scripts/build.sh
godot --path godot -- --local-cells

bash scripts/build.sh --features lbm-gpu
godot --path godot -- --local-lbm --lbm-validation --local-cells
```

原 loop lab 和 local Stokes 未接群体模式，组合入口明确拒绝。LBM仍只支持既有选定几何；本次没有放宽默认 small_world 的开口限制。

## CPU 真实交通回放

Intel Xeon Gold 6330，本机 release、既有四线程物理池、解析流、72总量。Full/Local 按同一条中心线路径通过普通 MoveInput 反馈控制机器人，各60模拟秒、3600步；没有直接移动物体位置。最终顺序执行，未同时运行项目构建、GPU探针或截图。逐帧CSV与摘要见 local-checks。复现：

```bash
cargo run --locked --release -p simulation --example circulation_probe -- 72 60 population-full /tmp/full.csv
cargo run --locked --release -p simulation --example circulation_probe -- 72 60 population /tmp/local.csv
```

| 指标（全部3600步，未丢弃预热） | Full | Local |
| --- | ---: | ---: |
| 实体软体范围 | 72 | 28–55 |
| solve均值 / p95 | 6.335 / 7.552 ms | 3.755 / 4.684 ms |
| snapshot均值 / p95 | 0.514 / 0.642 ms | 0.426 / 0.618 ms |
| 数量交接开销均值 / p95 / 最大 | 无 | 0.028 / 0.038 / 0.425 ms |
| 生成 / 退休 | 0 / 0 | 66 / 68 |
| 最大数量误差 | 0 | 2.84e-14 |
| 软接触帧数 | 3417 / 3600 | 3227 / 3600 |
| 面积比范围 | 0.95873–1.00033 | 0.95873–1.00069 |
| 最大节点壁面距离（负值为管内） | -0.09776 | -0.09677 |

本机这一次成对回放的solve均值降低约40.7%，p95降低约38.0%。Local的117个实际交接帧，整步p95为4.991ms、最大7.373ms。完整记录仍有长尾：Full最大95.867ms（frame3556），Local最大45.504ms（frame529，非交接帧）；尚未归因，不能只用均值宣称所有帧稳定。

逐帧：[Full CSV](local-checks/final-cpu-full.csv.gz)、[Local CSV](local-checks/final-cpu-local.csv.gz)；[摘要](local-checks/cpu-summary.json)。

这组比较减少了详细软体的数量，改变了远场物理近似，不能描述为相同数值任务的算法加速。全图墙体碰撞和背景几何仍保留；本次没有墙体装卸。

CPU探针没有新增通用分段计时接口，统计 solve、snapshot、exchange；Rapier/背景采样的独立分摊没有本机成对证据。LBM probe继续记录已有stage字段，但软件适配器短测不作为性能成绩。

## LBM 与 Godot 功能检查

Full/Local 使用 llvmpipe，各执行2帧既有 Fine 分叉验证场。两者72个初始软体、流体与机械守卫通过；Local总量72、详细72、库存0，四个个体速度/位移字段留空。这里的初始密集场没有减少软体，不能据此承诺性能改善。没有覆盖 LBM 与细胞交接同时发生的长程迁移。复现命令：

```bash
cargo run --locked --release -p simulation --features lbm-gpu --example lbm_gameplay_probe -- \
  --frames 2 --warmup 0 --case hold --allow-software --functional --local-cells --output /tmp/lbm-local
# Full去掉 --local-cells，另用输出目录。
```

原始记录：[Local](local-checks/lbm-local/status.txt)、[Full](local-checks/lbm-full/status.txt)。

Godot默认地图 [Full](local-checks/full.png)/[Local](local-checks/local.png) 均在120步截图，当前viewport、相机和静态缓存一致；已查看画面和数量标签，未见缺失可见细胞或新黑块。Local初始详细40、未实体化32、总量72。截图采用Xvfb/llvmpipe，只证明本地显示，不作为实卡FPS或精确轨迹等价证据。没有自动键鼠、点击、表单或按钮交互。

## 检查与审阅

- `bash scripts/check.sh`：格式、workspace编译、默认simulation检查、bridge构建、cached editor导入、world/lab headless运行通过。
- `cargo check --locked -p godot_bridge -p simulation --features lbm-gpu --all-targets` 通过；收尾feature bridge检查通过。
- `cargo test --locked -p simulation --features lbm-gpu --lib`：23通过、9个显式GPU用例忽略；不是9项GPU已验证。
- 数量与runtime七个相关用例通过：停流、旧库存输运、分叉、分离区间/移窗、真实移除/合法构造/受阻、暂停/单步/reset、反向流生成顺序。沿用原软体压缩恢复检查，不在Godot重复数量算法测试。
- world/world_hud/lab三个GDScript check-only、本地 `--local-cells` headless180步和不支持入口拒绝检查通过。

独立只读审阅发现反向流候选顺序问题：原逻辑先选低弧长端，在负Q时可能从出口侧构造。已用真实候选函数复现失败，按Q符号调整顺序后通过；没有为此新增库存或队列。上游受阻后可使用同区间的下游合法外缘，属于已确认的混合库存近似，不声称严格封堵。[项目检查](local-checks/project-check.log)、[feature检查](local-checks/feature-tests.log)、[审阅修补检查](local-checks/review-green.log)及[来源哈希](local-checks/source-hashes.json)随结果归档。

## 明确限制

普通远场细胞不保留身份、变形或密度团精确旅行时间；混合库存可能有数值扩散及阻塞后的空间重分布。没有实现严格堵塞或全局反压。72/80框仅覆盖当前固定视口/缩放和现有48×48 LBM，变焦、超宽窗口或更大流体域需重新确定范围。

未做RTX3060同机Rapier/LBM/完整渲染帧对照、LBM长程交接与移窗联合检查，也未完成人工WASD推挤、暂停/重置手感和大幅视口调整验收。它们是剩余验证项；不以本机CPU收益推断70ms整帧已解决，也不自动启动下一轮云实验。
