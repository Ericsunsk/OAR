# OAR 文档目录

当前文档采用“根目录扁平 + reference 子目录”的结构，降低查找成本，同时保留 source-of-truth 决策与阶段验证信息。

## 阅读顺序

1. [项目概览](project-overview.md)
2. [复盘收件箱](review-inbox.md)
3. [系统架构](system-architecture.md)
4. [执行与审计边界](execution-audit.md)
5. [记忆与证据链](memory-evidence.md)
6. [验证计划](validation-plan.md)
7. [飞书集成验证（Phase 0.5）](feishu-integration.md)
8. [身份与同步验证（Phase 0.6）](identity-auth-sync.md)
9. [参考资料](reference/references.md)

## 目录说明

| 路径 | 内容 |
| --- | --- |
| [`project-overview.md`](project-overview.md) | 项目定位、路线图、核心决策和阶段状态（合并简报/市场/产品总纲） |
| [`review-inbox.md`](review-inbox.md) | MVP PRD + 产品体验与工作流 |
| [`system-architecture.md`](system-architecture.md) | Swift / Rust / LarkAdapter、数据与运行时架构 |
| [`execution-audit.md`](execution-audit.md) | ConfirmedAction、OperationLedger、AuditEvent、安全与权限边界 |
| [`memory-evidence.md`](memory-evidence.md) | 三层记忆、证据链与治理原则 |
| [`validation-plan.md`](validation-plan.md) | 阶段门、验证实验、成功指标和停止标准 |
| [`feishu-integration.md`](feishu-integration.md) | Phase 0.5：Lark CLI / OKR 能力验证结论 |
| [`identity-auth-sync.md`](identity-auth-sync.md) | Phase 0.6：identity、token refresh、sync、idempotency、audit 验证 |
| [`reference/references.md`](reference/references.md) | 外部参考链接与资料索引 |
| [`reference/dependency-radar.md`](reference/dependency-radar.md) | 候选依赖雷达（非最终架构决策） |
