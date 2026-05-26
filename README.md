# OAR

OAR 是面向飞书企业租户的 OKR 复盘驾驶舱：每周自动发现 OKR 执行风险，汇总证据，生成行动建议，并在用户确认后安全写回飞书。

当前阶段：阶段 0.5 Lark CLI / OKR 能力验证已完成，阶段 0.6 身份、授权、同步和幂等执行验证进行中。

## 文档阅读顺序

1. [一页版项目简报](docs/one-page-brief.md)：快速理解 OAR 是什么、解决什么问题、当前最大风险是什么。
2. [产品总纲](docs/product-plan.md)：产品定位、MVP 范围、关键假设和文档地图。
3. [市场与定位](docs/market-and-positioning.md)：目标用户、市场判断、竞品和长期定位。
4. [产品体验](docs/product-experience.md)：macOS、iOS、飞书入口和核心工作流。
5. [技术架构总览](docs/technical-architecture.md)：Swift/Rust 架构、`LarkAdapter`、身份、同步和智能体运行时。
6. [安全、权限与执行边界](docs/security-and-permissions.md)：执行安全、人机分工、权限边界和数据边界。
7. [记忆架构](docs/memory-architecture.md)：三层记忆架构、检索流程、MVP 范围和治理原则。
8. [阶段 0.5 Lark CLI 验证报告](docs/phase-0.5-lark-cli-validation-report.md)：OKR 读取、dry-run、progress 创建 / 更新能力实测结论。
9. [阶段 0.6 身份与同步验证报告](docs/phase-0.6-identity-sync-validation-report.md)：身份、授权、refresh 前置条件、多端同步和幂等执行验证状态。
10. [验证计划](docs/validation-plan.md)：路线图、验证实验、成功指标、风险和停止标准。
11. [参考资料](docs/references.md)：竞品、飞书、Lark CLI、A2A 等外部参考来源。

## 下一步

1. 固化阶段 0.5 的 OKR CLI 输出，作为 `LarkAdapter` parser 和 fixture 的回归样本。
2. 完成阶段 0.6 的 `TokenGrant`、`DeviceSession`、`OperationLedger` 和 `AuditEvent` 骨架验证。
3. 初始化 Swift 前端与 Rust 后端，优先搭建复盘收件箱、幂等执行状态机和审计基础骨架。
