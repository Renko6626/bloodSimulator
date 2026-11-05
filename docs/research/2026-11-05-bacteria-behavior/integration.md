# 基础细菌行为合并记录

2026-11-05，用户明确要求合并回 main，然后提交推送。

- 来源：`feat/bacteria-behavior` 的 `f662c45`。
- 主工作树：`/data/sunyunbo/www/bloodSimulator`，原 main 为 `0bd496a`。
- `git fetch origin` 后远端 main 为 `f13001c`，没有远端独有提交。
- `git merge --ff-only feat/bacteria-behavior` 成功，无冲突；产品代码与已审阅分支完全一致。
- 本次另存集成记录提交，同步目标为 `origin/main`。宿主管理的功能工作树和分支保留。

## 主目录复核

以下命令均退出 0，编译及启动输出见 [日志](logs/main-integration/check.log)：

```bash
git diff --exit-code f662c45 -- crates godot scripts Cargo.toml Cargo.lock
cargo fmt --all --check
cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu
bash scripts/build.sh
godot --headless --path godot --fixed-fps 60 --quit-after 10 -- --combat-preview
```

桥接库已在主目录重新构建，战斗预览启动无脚本错误。记录提交前另运行 `git diff --check` 与 `git diff --cached --check`。

本次无产品代码调整，沿用功能分支已有64单元＋5集成、场景加载和独立审阅证据，不重复全套测试。人工手感与FPS仍未验收；未启动性能、烘焙或云实验。

现在可直接从主目录运行：

```bash
bash scripts/run.sh -- --combat-preview
```

行为配置与实际验收边界见 [阶段记录](README.md)。
