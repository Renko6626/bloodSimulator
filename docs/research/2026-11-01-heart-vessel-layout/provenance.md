# 来源与重现

首轮地图来自Task1提交 `7dad8730ef4a532173810d931897957cb5713adf`；首次真实回放发现最终回房导航漏掉实际血管折点。Task1随后提交 `5ea9431b2a0a40b127bde744415e1a258d73631d`，仅给路线补(38,32)并增加生产SDF连续路线检查，几何/图求解/默认配置未改。Task2只改诊断例子和交付文档。

| 源码 | SHA-256 |
| --- | --- |
| 首轮 `heart.rs` | `501e775e7e8ea4b5d5f732eb12e0b445e60470ce7dca635ce2afdb0336f52000` |
| 修正后 `heart.rs` | `eaea70b2e759737567ad61e4859c896742ea73f45d54510a66c750fb5cccccbf` |
| 首轮 `heart/vascular.rs` | `65177f0f01624d08fe50e63a0253e6241dd249c7e00533844728de344c83c29a` |
| 修正后 `heart/vascular.rs`（仅测试改变） | `735ced547618c6a80799b129d00795131e86aeb0e83d5b807983ed859edd0f15` |
| `crates/simulation/src/circulation/graph.rs` | `82ff6b7798d79db0ac5d111aa734fa1c380626b0e9eb476ab60e76f93ac1fe09` |
| `crates/simulation/src/config.rs` | `2ec94c307a64a53f3ac12a0e51f0055696b1497e9a535fd643e537a8fb685fd6` |
| 首次实际长回放的probe源码 | `e62041d81811f6fdb417127ebd63364f5fb1435881828fc019262a9810c1329b` |
| 标题/诊断选项修正后的中间probe源码 | `f72fcabf93a54cfb97e44772e0a031031134f1ac47047561d4a487197eddcddf` |
| 修正后完整回放及最终probe源码 | `7e66ebc0ae7f2ff4e0698bdedc0b1fe97b91631a1ef2d04276823f9d3f8ccc8c` |

[首次回放源码快照](probe-replay-source.rs.txt)精确匹配上述首次hash。回放运行期间，仅将导出图标题移至留白并加 `--diagnostics-only` 选项，未改变任何移动、守卫或数值求解逻辑。标题的max()初次缺少浮点类型注解导致E0689；实际日志和命令退出记录均保留。修正后完整回放使用[最终源码快照](probe-corrected-replay-source.rs.txt)，在原有每帧循环新增相同1.0阈值的及时失败断言，最终净空/峰值守卫保持。重新进行完整实际回放是为了验证已发生的路线问题，未因标题或选项变化重跑完整路线。最终图和CSV在修正后完整回放启动时再生成，几何/求解器/粘度与首轮一致。

在工作树根目录执行（所有cargo均复用主目录target）：

```bash
export CARGO_TARGET_DIR=/data/sunyunbo/www/bloodSimulator/target
cargo run --release --locked -p simulation --example heart_preview_probe -- docs/research/2026-11-01-heart-vessel-layout
bash scripts/check.sh
cargo test --locked -p simulation --features lbm-gpu
cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu
godot --headless --path godot --fixed-fps 60 --quit-after 180 -- --heart-preview
```

只重新生成源图/逐边CSV，无移动回放：

```bash
cargo run --release --locked -p simulation --example heart_preview_probe -- docs/research/2026-11-01-heart-vessel-layout --diagnostics-only
```

PNG由同名实际SVG以ImageMagick静态栅格化（没有手工重画管道或叠加流场）：

```bash
convert -background none -density 300 overview.svg -resize 900x1800 overview.png
```

`heart-ports`、`lung-micro-detail`同法。实际SVG/PNG已静态查看，无浏览器自动UI流程。概览/微管图的每根实线为真实中心线的定宽圆角胶囊；心腔为实际采样轮廓。SVG标题和CSV说明区分参考图流量与运行零流。

日志保留实际命令输出；仅去除四个检查/加载日志末尾多余空行，以通过仓库diff空白检查。
