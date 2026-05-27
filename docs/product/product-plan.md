# OAR 产品总纲

更新日期：2026-05-26
当前状态：方向判断 + MVP 定义 + 阶段 0.5 已通过 + 阶段 0.6 身份与同步验证中

## 0. 核心结论

OAR 值得继续做，但它不应该是通用 OKR SaaS，也不应该是通用智能体桌面。

最清晰的产品切口是：

> 面向飞书企业租户的 **OKR 复盘驾驶舱**：每周自动发现 OKR 执行风险，汇总证据，生成行动建议，并在用户确认后安全写回飞书。

长期形态是：

> 飞书 OKR 的 **AI 幕僚长 + 智能体网关**：负责盯进展、找风险、起草动作，并让其他智能体在 A2A 边界内协作；关键写回永远由人确认。

一句话原则：

> 不是让用户“管理 OKR”，而是让用户每周 10 分钟清空 OKR 执行风险队列。

## 1. 产品决策

| 主题 | 决策 | 说明 |
| --- | --- | --- |
| 产品定位 | OKR 复盘驾驶舱 | 聚焦飞书内的 OKR 执行运营，不做通用 OKR SaaS |
| 权威数据源 | 飞书 OKR | OAR 不替代飞书，只做智能体工作流层 |
| MVP 场景 | 每周 OKR 复盘 | 高频、可验证、能体现智能体价值 |
| 默认入口 | 复盘收件箱 | 避免变成低频仪表盘 |
| 写回策略 | 先读后写、写前预演、执行前人工确认 | 建立企业信任和审计边界 |
| 技术主线 | Swift 原生前端 + Rust 后端/core + LarkAdapter | 保持原生体验、强类型工具层和可审计执行 |
| Lark 集成 | Rust 原生 OpenAPI adapter 主路径；Lark CLI 仅用于本地验证、fixture 和回归 | 生产不引入跨语言 SDK bridge；飞书调用统一收敛在 adapter 层 |
| 7x24 运行 | 后端智能体运行时，而不是桌面端常驻 | 客户端负责交互和审批，后台负责调度、同步、审计 |
| A2A | 阶段 3 以后只读开放 | MVP 先闭环飞书 OKR，避免过早扩大安全面 |

## 2. 当前关键假设

| 假设 | 状态 | 当前判断 |
| --- | --- | --- |
| 飞书 OKR 仍是主要权威数据源 | 公开资料支持 | 成立，OAR 应叠加在飞书之上 |
| Lark CLI 可作为验证和 fixture 工具 | 已初步验证 | 阶段 0.5 价值成立；生产主路径转向 Rust 原生 OpenAPI adapter |
| `lark-okr` 覆盖 OKR 周期、Objective、KR、进展记录 | 已实测验证 | 阶段 0.5 已验证读取、dry-run、progress 创建 / 更新和删除 dry-run |
| Lark OAuth 可绑定用户身份 | 已初步验证 | 当前 user / bot 身份可被服务端验证，`auth:user.id:read` 已授权 |
| 后台 7x24 可代表用户工作 | 部分前置条件成立 | `offline_access` 已授权，但生产级 `TokenGrant`、refresh rotation、幂等执行仍待实现 |
| 用户愿意每周打开复盘收件箱 | 未验证 | 需要 3-5 个真实经理 / PMO 做 2-4 周陪跑式 MVP |
| 记忆能显著提升建议质量 | 未验证 | 需要历史 OKR 复盘回归用例 |

当前最重要的下一步：

> 阶段 0.6 已从“纯骨架”进入“过渡态验证”：token refresh service、Postgres Recorder、audit 写入与 `run_once` 幂等链路已有部分实测，但生产闭环尚未完成，需要继续打通真实 AuthAdapter/client、后台调度和多端一致性。

## 3. MVP 定义

第一版只验证一个场景：

> 每周打开 OAR，10 分钟处理完所有 OKR 风险和待确认动作。

MVP 必须包含：

- 从飞书读取 OKR 周期、Objective、KR 和 progress。
- 识别长期未更新的 KR、低进度 KR、缺少更新的 owner。
- 聚合 Docs、Tasks、Meetings、Minutes、Calendar、IM 中的相关证据摘要。
- 生成每周简报、风险队列和建议动作。
- 用户确认后写回进度、评论、提醒、任务或会议草稿。
- 记录 `AuditEvent`，可追溯 actor、scope、target、before/after 和执行结果。

MVP 不做：

- OKR 创建器。
- 完整组织树和复杂仪表盘。
- 绩效评价。
- 自动批量写回。
- 通用智能体市场。
- 外部 A2A 写回飞书。

第一版交付物：

- macOS OKR 复盘驾驶舱。
- Rust 后端/core 的 `LarkAdapter`、智能体运行时、审计、队列。
- 飞书 OAuth 登录和 identity binding。
- 阶段 0.5 Lark CLI 能力验证报告。
- 1 个真实团队跑通 2-4 周每周复盘循环。

## 4. 文档地图

| 文档 | 作用 |
| --- | --- |
| [`one-page-brief.md`](one-page-brief.md) | 给外部读者或设计伙伴快速理解 OAR 的一页版项目简报 |
| [`prd.md`](prd.md) | 第一版 OKR 复盘驾驶舱的用户、范围、需求、验收标准和风险 |
| [`market-and-positioning.md`](market-and-positioning.md) | 市场判断、竞品、目标用户、长期定位 |
| [`product-experience.md`](product-experience.md) | 桌面端、iOS、飞书入口和核心工作流 |
| [`technical-architecture.md`](../architecture/technical-architecture.md) | Swift/Rust 架构、LarkAdapter、身份、同步、智能体运行时 |
| [`security-and-permissions.md`](../architecture/security-and-permissions.md) | 执行安全、人的授权责任、智能体能力边界、权限和数据边界 |
| [`memory-architecture.md`](../architecture/memory-architecture.md) | 三层记忆架构、检索流程、MVP 范围和治理原则 |
| [`phase-0.5-lark-cli-validation-report.md`](../validation/phase-0.5-lark-cli-validation-report.md) | 阶段 0.5 Lark CLI / OKR 能力实测报告 |
| [`phase-0.6-identity-sync-validation-report.md`](../validation/phase-0.6-identity-sync-validation-report.md) | 阶段 0.6 身份、授权、同步和幂等执行验证报告 |
| [`validation-plan.md`](../validation/validation-plan.md) | 路线图、验证实验、成功指标、风险和停止标准 |
| [`references.md`](../reference/references.md) | 竞品、飞书、Lark CLI、A2A 等参考来源 |

## 5. 接下来 7 天

1. 把 `TokenGrant` refresh 与撤销链路从验证态推进到可持续运行，补齐 Rust 原生 OpenAPI `AuthAdapter` / client 调用路径与失败恢复。
2. 继续验证 Postgres Recorder + `OperationLedger` + `run_once` 的执行一致性，覆盖重试、超时和重复提交场景。
3. 收敛审计事件写入模型，确保关键动作都能追溯 actor、scope、结果与错误上下文。
4. 把后台 scheduler / daemon 接到真实任务流，在客户端离线时验证任务仍可按策略运行。
5. 用真实团队复盘原型做多端状态同步回归（macOS / iOS / 飞书入口），确认“生产闭环未完成但方向有效”。

## 6. 未来判断

**有未来，但它不是一个大众软件机会，而是一个垂直企业工作流机会。**

它的未来取决于三个判断是否成立：

1. OKR 的真正痛点不是创建，而是执行运营。
2. 飞书生态足够封闭且足够丰富。
3. 智能体的价值在“起草 + 证据 + 确认”，不是全自动。

明确判断：

> 值得继续做，但第一版必须极窄：只做每周 OKR 复盘驾驶舱。先证明“每周 10 分钟清空 OKR 风险队列”这件事有人愿意持续使用，再谈 A2A 和更大的智能体中枢。

如果每周循环跑通，OAR 有机会从一个飞书内部工具长成 OKR 智能体中枢；如果跑不通，继续加智能体、A2A、原生端都只是把复杂度堆高。
