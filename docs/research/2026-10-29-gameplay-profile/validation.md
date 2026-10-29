# 实际验证命令与结果

这些命令用于诊断源码副本。正式 simulation/bridge/Godot 源码未改，因此未扩大到默认游戏的全量交互验证，也未新增永久测试。

## 编译与脚本

```bash
cargo build --offline --locked --release -p godot_bridge --features lbm-gpu --manifest-path /tmp/sunyunbo/lbm-profile-20261029/source/Cargo.toml --target-dir /tmp/sunyunbo/lbm-profile-20261029/target
cargo build --offline --locked --release -p simulation --features lbm-gpu --example lbm_gameplay_probe --manifest-path /tmp/sunyunbo/lbm-profile-20261029/source/Cargo.toml --target-dir /tmp/sunyunbo/lbm-profile-20261029/target
```

均 exit0。诊断 bridge/probe 已上传，二进制 SHA 见 manifest；原 baseline SHA 与上轮封存相同。

```bash
XDG_DATA_HOME=/tmp/sunyunbo/lbm-profile-20261029/userdata XDG_CONFIG_HOME=/tmp/sunyunbo/lbm-profile-20261029/config XDG_CACHE_HOME=/tmp/sunyunbo/lbm-profile-20261029/cache godot --headless --path /tmp/sunyunbo/lbm-profile-20261029/source/godot --script res://world.gd --check-only --log-file /tmp/sunyunbo/lbm-profile-20261029/check-world.log
cc -shared -fPIC -O2 -Wall -Wextra -o /tmp/sunyunbo/lbm-profile-20261029/wait-trace-final.so docs/research/2026-10-29-gameplay-profile/wait-trace.c -ldl -pthread
bash -n docs/research/2026-10-29-gameplay-profile/run-hardware.sh docs/research/2026-10-29-gameplay-profile/run-focused-controls.sh docs/research/2026-10-29-gameplay-profile/run-clock-controls.sh
```

均 exit0。各 Python 分析脚本用 `ast.parse` 检查语法，并实际读取原始结果运行，未生成配套测试工程。

## 功能 smoke 与实卡

软件适配器分别运行 probe/Godot 1 预热＋2 测量步，COMPLETE/exit0。Godot 使用诊断副本专属的 `BLOOD_ALLOW_SOFTWARE=1`；没有将软件运行时间列为实卡性能。

实卡在 `/workspace/lbm-profile-20261029` 执行：

```bash
export GODOT_BIN="$PWD/godot-4.7.1" DISPLAY=:99
timeout --kill-after=15 900 bash run-hardware.sh
timeout --kill-after=15 420 bash run-focused-controls.sh
timeout --kill-after=15 500 bash run-clock-controls.sh
```

三个 runner 均 exit0。主矩阵、聚焦控制和延长控制的全部实际 probe/Godot 运行均 COMPLETE/exit0。首次无缓存 editor import 的两个 exit134 独立保留，不计作通过。没有 UI 自动点击、填写表单或新后端测试。

## 分析和证据核验

仓库根目录执行：

```bash
python3 docs/research/2026-10-29-gameplay-profile/summarize-baseline.py docs/research/2026-10-29-lbm-gameplay/hardware-rtx3060/results docs/research/2026-10-29-gameplay-profile/matched-baseline.json
python3 docs/research/2026-10-29-gameplay-profile/analyze-profile.py docs/research/2026-10-29-gameplay-profile/hardware-rtx3060/verified/results docs/research/2026-10-29-gameplay-profile/profile-summary.json
python3 docs/research/2026-10-29-gameplay-profile/analyze-clocks.py docs/research/2026-10-29-gameplay-profile/hardware-rtx3060/verified/results docs/research/2026-10-29-gameplay-profile
python3 docs/research/2026-10-29-gameplay-profile/analyze-syscalls.py /tmp/sunyunbo/lbm-profile-20261029/focused/results/syscalls.log docs/research/2026-10-29-gameplay-profile/syscalls-summary.json
git diff --check
```

分析均 exit0。全部短程案例共同记录的帧前缀中，12 个记录量 f32 比较无差异；延长 probe/Godot 对应 2520 步也无差异。仅比较记录量，不是全内部状态逐位等价证明。

结果包 SHA 与远端一致，所有实验结果文件 SHA 通过。持续运行的 Xorg 在清单生成后继续写其日志，唯一不一致为 `display/Xorg.99-fixed.log`；详见取回核验 JSON。完整 syscall 日志只保留在原始压缩包中，展开目录省去该 22 MB 副本。

停机 API 返回 success 后又核对实例状态，最终 `58212504: stopped/exited`；磁盘未销毁。

未验证：用户桌面环境、正式优化后的画质/性能、GL GPU 时间线、固定时钟下的因果分解，以及首次 editor 导入退出异常的根因。
