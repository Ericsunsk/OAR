# OAR

OAR 当前处于产品与技术规划阶段，目标是构建一款基于 Lark CLI、面向 AI 智能体的 OKR 复盘驾驶舱，采用 Swift 原生前端与 Rust 后端。

## 当前文件

- `docs/product-plan.md`：产品定位、MVP、技术路线、身份/同步、智能体运行时、A2A 边界。
- `docs/human-role.md`：人在 OAR 中的作用、智能体边界和人机协作原则。
- `docs/lark-cli-capability-matrix.md`：阶段 0.5 Lark CLI / `lark-okr` 能力验证清单。
- `.gitignore`：Swift、Rust、本地环境文件忽略规则。

## 下一步

1. 准备真实飞书测试租户、测试用户和一次性测试 Objective / KR。
2. 安装并登录 `lark-cli`，执行 `docs/lark-cli-capability-matrix.md` 中的 `T0-T6`。
3. 根据验证结果决定 `LarkAdapter` 主路径：`lark-okr`、OpenAPI 兜底路径，或只读 MVP。
4. 初始化 Swift 前端与 Rust 后端，优先搭建复盘收件箱、LarkAdapter、智能体运行时和审计基础骨架。
