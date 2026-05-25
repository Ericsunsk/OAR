# OAR

OAR 当前处于产品与技术规划阶段，目标是构建一款基于 Lark CLI、面向 AI Agent 的 OKR Review Cockpit，采用 Swift 原生前端与 Rust 后端。

## 当前文件

- `计划.md`：产品定位、MVP、技术路线、身份/同步、Agent Runtime、A2A 边界。
- `docs/lark-capability-matrix.md`：Phase 0.5 Lark CLI / `lark-okr` 能力验证清单。
- `.gitignore`：Swift、Rust、本地环境文件忽略规则。

## 下一步

1. 准备真实飞书测试租户、测试用户和 disposable Objective / KR。
2. 安装并登录 `lark-cli`，执行 `docs/lark-capability-matrix.md` 中的 `T0-T6`。
3. 根据验证结果决定 `LarkAdapter` 主路径：`lark-okr`、OpenAPI fallback，或 read-only MVP。
4. 初始化 Swift 前端与 Rust backend，优先搭建 Review Inbox、LarkAdapter、Agent Runtime 和 Audit 基础骨架。
