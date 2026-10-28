# LBM 可玩验证场与机械耦合性能实测

2026-10-29。用户已确认先做可玩的 LBM 分叉验证场，接入真实机器人和红细胞，再测固定窗口与玩家移动/窗口迁移。以 `14bf6d3` 为基线；沿用现有 Rust/wgpu、有限阻力机械交换、Rapier 与 Godot 架构，不另换耦合算法。

## 实施范围

1. 复用已验证 fork_a 的局部直支路几何，在测试窗口外闭合血管回路、放置泵段，由现有 GraphNetwork 解流量；记录实际流量，不把原先手给 70:30 当作本轮图求解结果。验证场采用明确入口，默认 small_world 不改。
2. 真实 72 个十二节点红细胞，尽量集中在窗口附近但不以严重初始穿插制造负载；保留材料参数、拖曳和玩家控制。验证场的 reset 应重建同样场景。
3. 沿用现有桥接添加验证场启动选择。玩家可以正常 WASD 操作，性能探针通过相同 MoveInput 路径重放明确路线，不瞬移机器人、不直接改粒子轨迹。
4. 检查固定点及移动路线涉及的开口、LSQ 与迁移；当前几何不支持时报告原因，不放宽约束或静默回退。
5. release 实卡先做短 smoke，再运行固定/移动两种各 60 模拟秒（预热 2 秒，测量 3600 帧）的 72 细胞案例；必要时只针对实际失败修正/缩小可用路线并保留失败证据。不扩成分辨率/参数大矩阵。
6. 逐帧记录 simulation wall、prepare/coupling/LBM/Rapier、读回与迁移、实际玩家位移、活跃细胞、接触见证、密度/流量、细胞有限性/面积/越壁。重诊断置于计时区外，分别记录 snapshot 成本；绝不以 commanded movement 冒充实际移动，也不以 cell_count 冒充实际拥挤。
7. 尽可能用同一验证场运行 Godot 性能回放并取得一张截图；记录渲染器、GPU 与显示环境。仅 headless/无渲染结果不能称为完整渲染帧性能。输入回放属于用户明确要求的玩家移动实测，不自动操作 UI 按钮/表单。

## 分工与验证

- 既有 Sol high 子代理负责验证地图、局部场景初始化、现有游戏入口和最小探针；只改相应 crates/simulation、crates/godot_bridge、godot 下文件，不改已验证 LBM 数值算法或研究存档。
- 主代理负责云实例身份核验/启停、部署冻结产物、运行与取回、结果分析、报告和入口文档。无新依赖或模块边界；出现超出既定路线的决策先说明。
- 相关已有检查和最小新增地图/重置/路线行为验证，feature 编译集中执行，不重复无关全量测试。
- 原始结果、实际命令、源码/binary 哈希保存至 `docs/research/2026-10-29-lbm-gameplay/`。实例完成所有实验、取回并核验后 stop 并确认 stopped/exited，保留磁盘。

## 状态

实卡新增证据：Rust 两例已完成，Godot 开启默认 GL 绘制后的实际帧间隔显著增大，并伴随 LBM 读回等待增加。为解释这一实际差异，追加同一冻结代码/物理的两个有界控制：headless 与保留 GL 上下文但 `--disable-render-loop`，各 120 预热＋600 测量移动帧。与正式 move 的相同前缀比较，保留全部结果，不改变物理参数、不推广新渲染后端。

上述控制完成后，Godot 自带 help 明确指出 `--disable-vsync` 不覆盖驱动强制同步；最后追加一次同前缀的 `__GL_SYNC_TO_VBLANK=0` 进程环境控制，排查驱动覆盖。只作用于该进程，不改全局驱动或频率，不继续扩展参数扫描。

- [x] 验证场、正常游戏入口与输入回放
- [x] 本机相关功能/编译检查
- [x] RTX 完整模拟固定/移动实测
- [x] Godot 运行检查、性能记录与截图（说明渲染边界）
- [x] 原始数据、结论、适用范围与停机核验


## 完成记录

- 验证场、正常Godot入口、72细胞局部seed/reset、Rust/Godot同MoveInput回放完成；数值算法、默认地图和材料参数未改。图实际分流56.94:43.06。
- 地图/seed与reset、软件smoke、release构建、Godot脚本check-only及feature开/关编译通过。主代理实际命令：`cargo check --locked -p simulation -p godot_bridge --no-default-features`；`cargo check --locked -p simulation -p godot_bridge --features godot_bridge/lbm-gpu`；`cargo fmt --all -- --check`。其他实际命令见[实现交付记录](../../research/2026-10-29-lbm-gameplay/local-checks/implementation.md)。
- 三项审阅问题及压力标签已局部修复；临时失败注入验证probe保留原始错误，不追加恢复机制。正式实卡前重新冻结source/probe/bridge，代码与归档source逐字核对。
- Rust probe及Godot的固定/移动四个主案例各3720总帧/3600测量帧，全部exit0、COMPLETE；Godot采用真实NVIDIA OpenGL/Xorg显示，截图已查看。三个有界控制各720总帧全部exit0。原始结果与报告见[实测报告](../../research/2026-10-29-lbm-gameplay/README.md)。
- 当前整体游戏性能未通过60FPS目标：Godot默认绘制帧间隔p95约70–71ms；不隐藏负面结果、不据此擅自更换渲染架构。首次headless editor导入exit134已独立复现，cached import及gameplay正常，根因未明。
- 结果全部取回并通过SHA，实例58133143确认stopped/exited。保留分支和磁盘，不自动合并/推送，不追加云端实验。

## 阶段归档

用户后续明确要求“阶段性保存整理，提交推送”，并指定“合并到 main”。本轮归档包含旧工作区遗留的 FD/NEQ 初始研究笔记，保留为历史计划并链接当前结果；实现和实验数据不改。验证文档链接、源码与实测冻结包一致性及结果 SHA 后，以 main 为合并和推送目标，不重新启动 GPU 或追加实验。
