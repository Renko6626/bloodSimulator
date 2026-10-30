# 区块流体策略功能交付

2026-10-30，分支 `feat/chunk-fluid-policy`。实现来源为 `918c641` 后五项计划任务；本报告与Task 5源代码同提交。软件回放构建基于 `1b86266` 加本次探针及面积检查修改，原始日志、600行逐帧记录和SHA256清单在 [software](software/manifest.json)。

已实现背景来源标签、固定黏度/端口比例静态场契约、新策略地图二维流量、驱动准备校验、完整3×3窗口许可及几何预检查、单窗口按帧创建/释放和统一空间权重。默认禁止LBM，旧无策略入口及固定负载保持原路线。预计算资产通过现有静态格点/插值支撑检查，不承诺解析追踪任意狭窄几何。

## 实际600帧回放

显式 `--fluid-policy` 使用原分叉地图的策略副本，只许可x=0..2、y=0..2九块，唯一有效中心(1,1)，禁区出生[14,24]。60帧保持，随后各180帧正常MoveInput位置反馈到[24,24]、[14,24]、[24,24]；没有瞬移或UI操作。使用正常策略构造器，72个红细胞含Aggregate库存，白细胞沿用普通入口开关，回放可见1个白细胞；未移除白细胞。共享面积检查按白细胞半径1、红细胞半径0.7计算参考面积，阈值不改。旧验证场种子/红细胞工作量不变；新策略入口使用普通世界种子分布，并非旧验证场的局部72个详细软体。

软件适配器为llvmpipe (LLVM 15.0.7, 256 bits)。真实观察：

| 事件 | 帧（从0开始） | 自机位置 | 窗口原点 |
|---|---:|---|---|
| 创建 | 87 | [16.090988,24.0] | [0,0] |
| 释放 | 375 | [15.8986435,24.027971] | 无 |
| 再创建 | 447 | [16.13861,24.003983] | [0,0] |

600步退出0，末位置[24.016376,24.002262]。159帧无窗口、441帧有窗口；窗口选择按帧开始位置核对，跨界当帧不能由移动后区块反推选择。每个活动窗口从snapshot原点还原中心，检查全部九块许可；禁区帧无窗口、activation/marker力/局部权重均为0。activation实际达到1，marker峰值0.743532。机械守卫全程通过：面积比范围0.945754～1.000188、节点墙距离最大-0.149974，自机墙距离最大-2.928589。红细胞详细数10～13、抽象数59～62，总数72，数量账最大误差1.4211e-14。

逐帧CSV还记录机器人速度及相邻帧速度差、机器人位置处局部/背景速度向量差的模长。后者在无窗口帧表示零局部速度与背景的差，不表示实际局部作用；以local_weight判断作用。机器人速度最大4.467916，速度差范围-0.522911～4.387207包含目标切换；这些仅为观测，不是物理精度判断。

禁区reset的显式ignored测试退出0：重置仍无窗口，保留LBM请求，后续正常输入实际创建窗口。

## 集中检查

所有Cargo命令设置 `CARGO_TARGET_DIR=/data/sunyunbo/www/bloodSimulator/target`。构建缓存仅复用，Godot库路径由忽略的本地target链接目录提供。

| 实际命令 | 结果 |
|---|---|
| `bash scripts/check.sh` | 退出134；fmt、workspace check、默认38单元+5集成测试及桥接构建通过，首次Godot editor import在layout加载后abort；脚本后续两项未执行 |
| `cargo test --locked -p simulation --features lbm-gpu` | 退出0；49单元+5集成通过，10项ignored未默认运行 |
| `cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu` | 退出0 |
| `cargo run --locked -p simulation --features lbm-gpu --example lbm_gameplay_probe -- --fluid-policy --allow-software --functional --frames 600 --output /tmp/blood-chunk-fluid-policy` | 退出0，真实创建→释放→重建通过 |
| `cargo test --locked -p simulation --features lbm-gpu forbidden_policy_reset_retains_request_and_later_creates_window -- --ignored --exact lbm_validation::tests::forbidden_policy_reset_retains_request_and_later_creates_window` | 退出0；1项真实功能测试通过 |
| `godot --headless --path godot --editor --import --quit` | 缓存导入复查退出0 |
| `godot --headless --path godot --fixed-fps 60 --quit-after 180` | 退出0 |
| `godot --headless --path godot --fixed-fps 60 res://lab.tscn --quit-after 180` | 退出0 |

首次导入失败未被掩盖：按systematic-debugging读取日志，失败发生在Godot编辑器导入退出阶段，没有Rust panic或模拟错误。既往归档也有首次导入异常而缓存导入/真实运行成功的记录；本次最小复查仅重跑失败导入及未执行场景，不重跑全部检查、不修改产品以绕开错误。根因未确定，不能声称全脚本通过。

## 只读审阅与边界

核对完整改动的地图默认许可、背景二维驱动校验、预计算限制、候选中心、帧选择/释放/reset和耦合采样路径，未发现重复推进或新增隐藏回退。候选几何不适合会在准备阶段记录拒绝；真正GPU创建、prepare或solve失败继续向调用者返回错误。旧探针默认分支只新增显式旗标分流，旧720帧每腿逻辑保持；background.rs额外差异仅rustfmt。

本轮完成区块基础设施和Rust软件功能证据。心腔生成、静态场生成算法、大地图内容未实施；未测实卡FPS、长程混合稳定性、任意斜率几何或物理精度，未启动云实例、自动UI交互、推送、合并或部署。
