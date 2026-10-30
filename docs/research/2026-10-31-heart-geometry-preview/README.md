# 四腔心脏几何预览

本阶段在 `feat/heart-geometry-preview` 实现可探索的静态四腔几何。基于 `5f28730` 的离线 LBM 基础设施；几何提交 `e4c49df`，场景与真实输入探针提交 `5e1c188`。本阶段不烘焙或连接心腔流场，没有细胞、收缩、动态瓣膜、单向通行或正式心腔阻力模型。

## 启动与范围

```sh
bash scripts/build.sh
godot --path godot -- --heart-preview
```

入口固定16单位块、5×5现场和zoom2，1280×800时视野约26.7×16.7单位；不会生成全图mesh。`--stream-world` / `--local-cells` 可冗余传入，预览最终使用零细胞配置。与 `--local-lbm`、`--local-stokes`、`--lbm-validation`、LBM profile、benchmark及其截图参数组合立即报错退出1。

WASD、调速、暂停、N单步、R重置及C碰撞显示保留。HUD明确显示预览、零流和零细胞，压力固定0并禁用，流向开关及F无效；细胞拖曳控件禁用。调速信号和桥接参数入口不会恢复非零压力，Rust预览标记在推进/重置时重新固定零压力、零脉动及零红/白细胞。默认世界/Local/LBM工厂沿用原路径。

## 几何与来源

四个独立闭合轮廓复用已有管道并集、权威SDF、分块墙和地面；新轮廓使用标准三次Bezier采样，不修改旧血管Bezier。右/左房室口、流出道以及四个外部接口静态开放；左右组织中隔保留，只有主体外肺/体循环连线形成外部闭环。

比例参考主目录 `.worktrees/heart-proportion-20261031/heart-proportion.html`，源轮廓和导航点以 `crates/simulation/src/map/heart.rs` 为准，按净空和可达性局部调整。参考图不作为实际游戏截图。主体约160×192量级；外循环连线超出主体。具名导航点只供认路，不构成墙。零压合法图仅供现有运行结构使用，不是心腔水动力学验收。

## 验证证据

实际命令、退出状态、探针输出见 [logs](logs/)。Rust探针从正常spawn以真实MoveInput沿右房、右室深部、右流出道、外部肺连接、左房、左室深部及主动脉出口推进，不瞬移。每段有界，并断言零细胞、零白细胞、零分支流、零总流和零压差。

集中检查全部通过：`cargo fmt --all --check`、`cargo test --locked -p simulation`（45单元＋5集成）、`cargo test --locked -p simulation --features lbm-gpu`（58单元＋5集成，10个已有忽略）、`cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu` 和 `bash scripts/check.sh`。本轮首次Godot导入退出0，无需重试；脚本内普通world/lab加载均退出0。

正常preview及冗余 `--stream-world --local-cells` 启动退出0；9种不兼容参数分别退出1，见 [startup-results.txt](logs/startup-results.txt)。最终HUD压缩预览统计后单独重跑cached import及preview加载，均退出0，未重复全套Rust检查。

探针实际到达全部19导航点，右心室纵向跨度83.985、左心室89.941；全程最小中心到墙净空2.790（tricuspid段），峰值25块，零数量/压差/流量断言全部通过，见 [heart-probe.log](logs/heart-probe.log)。这些跨度来自真实位置，不用轮廓AABB代替。

[真实预览静态截图](preview-startup.png) 来自本机 `DISPLAY=:2`、Godot4.7.1/OpenGL4.5、Mesa llvmpipe软件渲染；启动后未输入按键、点击或操控UI。仅捕获本次PID508754所属的1280×800游戏客户区，使用 `xprop` 核对PID后由 `xwd -id 0x3200003` 抓取并转换PNG，截图后仅终止本次进程。初次静态查看暴露预览统计使下方控件溢出，已仅压缩预览统计，最终保留本图；压力、拖曳和流向禁用，碰撞开关完整可见。没有以截图运行做性能结论。

## 验收边界

Godot验证仅加载、命令行参数检查及一张正常启动的静态截图；没有自动键鼠、点击、填写或UI流程测试。机器人穿行证据来自Rust真实生产推进，并不代表已人工体验Godot操作、墙边手感或全部路径。尚未验收心腔流场、绝对压力响应、细胞分布与运输、瓣膜机制、真实心脏物理精度或实卡FPS。没有云实例、性能研究或扩大实时LBM窗口。本地提交不包含推送或合并。
