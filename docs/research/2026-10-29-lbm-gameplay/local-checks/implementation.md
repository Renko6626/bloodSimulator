# LBM 可玩验证场：实现代理实际交付与验证

工作树 `/data/sunyunbo/.codex/worktrees/8d56/bloodSimulator`，分支 `feat/lbm-gameplay-validation`。本记录仅整理已执行证据，没有追加检查、数值运行或代码改动。云实卡生命周期与正式结果由主代理维护。

实现范围：显式验证场构造、远处闭合 GraphNetwork、局部 72×12 节点 cell seed、reset 保留 seed/当前 pressure、MoveInput 往返回放、专用 probe、Godot 入口/CSV/预热截图。未改 fluid-gpu 算法。审阅后修正远处支路反向重叠，并修复 probe 在失败 snapshot.lbm=None 时 unwrap 掩盖原始错误的问题；HUD 标定压力标签同步刷新。

最终本机构造：pressure=12.605889，图解 rates 前三项 `[18.529295, 10.55135, 7.9779463]`；分流约 56.94:43.06。初始 72 个 active cell，玩家 8 单位内 29 个；路线另一窗口 origin(-1,0) 的 raster/LSQ 预检可建立，初始 origin(0,0) 由实际 sim 建立。实际跨 block、完整负载与渲染吞吐以主代理实卡结果为准。

## 已执行检查

Cargo 命令均在上述工作树执行，使用 `CARGO_TARGET_DIR=/data/sunyunbo/www/bloodSimulator/target CARGO_BUILD_JOBS=2` 复用缓存。

- **CPU map/seed/flow**：`cargo test --offline --locked -p simulation --features lbm-gpu --lib graph_flow_and_local_seed_are_explicit`。通过（1 passed）。最终地图修正后已重跑该项，含远处 branch 与 return 中段保持分离、总 Q 与图守恒、72 中心间距/距壁/局部分布、回放方向断言。最初 `<-` 缺空格的 Rust 语法错误已修正后才通过。
- **GPU reset 功能检查**：`cargo test --offline --locked -p simulation --features lbm-gpu --lib validation_reset_preserves_seed_and_current_pressure -- --ignored --test-threads=1`。通过（1 passed）；验证 72 seed、已修改 pressure、paused 状态、验证地图保留。适配器是明确允许的 llvmpipe；该检查在远处回路折线审阅修正前执行，之后没有因仅改变远处几何重复 reset 检查。
- **初版软件 smoke**：`/data/sunyunbo/www/bloodSimulator/target/debug/examples/lbm_gameplay_probe --functional --allow-software --case hold --warmup 0 --frames 2 --output /tmp/lbm-gameplay-smoke`。exit0/COMPLETE；72 active，CSV header/两行数据均 44 列。旧图分流数据仅属这次初版，不作为最终图流量。
- **最终地图软件 smoke**：`/data/sunyunbo/www/bloodSimulator/target/release/examples/lbm_gameplay_probe --functional --allow-software --case hold --warmup 0 --frames 1 --output /tmp/lbm-gameplay-mapfix-smoke`。exit0/COMPLETE；最终 Q/pressure 如上。该案例构造时也实际建立另一 route 窗口的 LSQ/raster preflight。CSV/status 在该目录；该次终端输出保留于工具记录，未另保存 run.log。仅软件功能诊断，不是性能证据。
- **最终 fresh release build**：`cargo build --offline --locked --release -p godot_bridge -p simulation --features godot_bridge/lbm-gpu --lib --example lbm_gameplay_probe`。通过；最终集中修复后同时重建 bridge 与 probe。没有修改已冻结 binary 后再次补源码数值。
- **Godot script check-only**：在 `/tmp/lbm-gameplay-godot-smoke/godot` 使用项目副本，debug 路径指向上述 release bridge。实际命令分别为 `godot --headless --path /tmp/lbm-gameplay-godot-smoke/godot --script res://world.gd --check-only` 和同命令 `--script res://world_hud.gd`。最终脚本均 exit0。未在本机运行完整 Godot 软件 benchmark。
- **probe failure-path 注入**：复制真实最终 probe 到 `/tmp/lbm-gameplay-failure-probe.rs`，仅在 frame1 把 step 结果设为 `Err("injected_original_step_failure")` 且 snapshot.lbm=None、lbm_status 设为 pending-cache 错误。用 `rustc --edition=2024 --cfg 'feature="lbm-gpu"'`、现有 release dependency cache 及 simulation/blood_fluid_gpu 实际 rlib 编译；完整展开的 rlib 文件名未另保存，不编造。运行 `/tmp/lbm-gameplay-failure-probe --functional --allow-software --case hold --warmup 0 --frames 2 --output /tmp/lbm-gameplay-failure-check`。预期且实际 exit1；status 含原始注入 step error、missing-LBM 诊断、last有效 region_switches=0 和 result=Err，CSV prefix 已 flush，无 panic。日志 `/tmp/lbm-gameplay-failure-check/run.log`、status.txt。此为错误记录控制流注入，不能当作自然发生的流体失稳。
- **格式/whitespace**：修改时执行 `rustfmt --edition 2024` 于 `crates/simulation/src/{lbm_validation,runtime,lib}.rs`、`crates/simulation/examples/lbm_gameplay_probe.rs`、`crates/godot_bridge/src/lib.rs`（分次按实际改动格式化）。初次交付前同组文件 `rustfmt --edition 2024 --check ...` 通过；最终局部修正后执行相应 rustfmt，并执行 `git diff --check -- crates/simulation crates/godot_bridge godot/world.gd godot/world_hud.gd` 通过。没有为文档/路径/哈希再跑完整数值矩阵。

## 冻结产物

- `/data/sunyunbo/www/bloodSimulator/target/release/examples/lbm_gameplay_probe`
  SHA256 `4d3091583225593c15842443bb94360403268c6b6dc349e5de92883d92591a75`
- `/data/sunyunbo/www/bloodSimulator/target/release/libgodot_bridge.so`
  SHA256 `658519253874db43dc04479ebdddf269edbfbe9950f37fc5cf83eae8c92f636d`

CLI 不依赖外部 LSQ weights。正式 probe `--case both --warmup 120 --frames 3600 --output DIR`；正式硬件模式不加 software flags。Godot 参数由已归档主代理 runner 保存。

## fresh editor 导入退出异常（未修复）

最初项目副本 `godot --headless --editor --path /tmp/lbm-gameplay-godot-smoke/godot --import --quit` 在完成 filesystem/scripts/editor layout 后 exit134，无明确脚本报错。当时与重链接并行；后续有界复现证明不能据此归因并行重链接。

`/tmp/lbm-godot-editor-abort/` 中同命令对照：fresh empty 项目 exit0；fresh 项目仅加载同一冻结 extension（无 world script/scene）returncode -6/SIGABRT；已有 .godot 缓存后同 extension 项目 exit0；另一个 fresh extension 项目再次 -6。fresh 项目 gdb 捕获到主线程 Godot 可执行文件内 SIGSEGV，栈地址 `0x1c7697f → 0x1c781d4 → 0x1c80955 → 0x4e661dd → 0x495d7c4 → main`；二进制剥离符号，尚不能确定具体内部函数和根因。cached gdb 启动正常退出，无栈。

日志：`empty.log`、`extension.log`、`extension-cached.log`、`extension-fresh2.log`、`gdb-fresh.log`（主代理已另归档这些证据）。本问题限定为已观察的 fresh editor 导入/退出与 extension 组合；check-only exit0 和主代理真实 NVIDIA gameplay exit0 是不同证据，不声称 fresh import 已通过。没有为此修改运行架构、算法、Godot 版本或冻结产物。
