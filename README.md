# OAR

OAR 当前处于产品与技术规划阶段，目标是构建一款基于 Lark CLI、面向 AI 智能体的 OKR 复盘驾驶舱，采用 Swift 原生前端与 Rust 后端。

## 当前文件

- `docs/product-plan.md`：产品总纲、MVP 定义、关键假设和文档地图。
- `docs/market-and-positioning.md`：市场判断、目标用户、竞品和长期定位。
- `docs/product-experience.md`：桌面端、iOS、飞书入口和核心工作流。
- `docs/technical-architecture.md`：Swift/Rust 架构、LarkAdapter、身份、同步和智能体运行时。
- `docs/security-and-permissions.md`：执行安全、智能体能力边界、权限和数据边界。
- `docs/human-role.md`：人在 OAR 中的作用、智能体边界和人机协作原则。
- `docs/memory-architecture.md`：三层记忆架构、检索流程、MVP 范围和治理原则。
- `docs/lark-cli-capability-matrix.md`：阶段 0.5 Lark CLI / `lark-okr` 能力验证清单。
- `docs/validation-plan.md`：路线图、验证实验、成功指标、风险和停止标准。
- `docs/references.md`：竞品、飞书、Lark CLI、A2A 等参考来源。
- `.gitignore`：Swift、Rust、本地环境文件忽略规则。

## 下一步

1. 准备真实飞书测试租户、测试用户和一次性测试 Objective / KR。
2. 安装并登录 `lark-cli`，执行 `docs/lark-cli-capability-matrix.md` 中的 `T0-T6`。
3. 根据验证结果决定 `LarkAdapter` 主路径：`lark-okr`、OpenAPI 兜底路径，或只读 MVP。
4. 初始化 Swift 前端与 Rust 后端，优先搭建复盘收件箱、LarkAdapter、智能体运行时和审计基础骨架。
