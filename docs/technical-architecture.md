# 技术架构总览

更新日期：2026-05-26

## 1. 架构原则

OAR 的技术形态：

> Swift 原生前端 + Rust 后端/core + LarkAdapter + 服务端智能体运行时。

核心原则：

- 飞书是 OKR、Docs、Tasks、Meetings、Calendar、IM 的权威数据源。
- OAR 后端是复盘、待确认动作、审计事件、证据索引、记忆和同步游标的权威数据源。
- iOS / macOS / 飞书入口负责交互、查看和审批。
- 7x24 调度、同步、审计和工具执行在后端完成。

## 2. 原生技术形态

推荐实现：

| 层 | 技术 | 说明 |
| --- | --- | --- |
| macOS | SwiftUI + AppKit bridge | 原生窗口、sidebar、command menu、通知、deep link |
| iOS | SwiftUI | 轻量审批和提醒，不承载完整智能体运行时 |
| 后端 / Core | Rust | LarkAdapter、队列、审计、策略、工具执行、同步引擎 |
| 本地通信 | local HTTP / gRPC / XPC | macOS shell 可连本地 core；生产更推荐后端服务 |
| 服务端 | Rust service | 7x24 任务、OAuth token、同步、A2A server、审计存储 |
| 存储 | Postgres + 对象存储 + 向量索引 | 审计、待确认动作、证据摘要、记忆 |

关键判断：

- iOS 不适合作为 7x24 智能体执行环境，只作为审批和查看端。
- macOS 可以有本地 core，但不能依赖用户电脑在线完成组织级复盘。
- 真实 7x24 能力必须在服务端智能体运行时。
- Swift 前端只负责体验和状态展示，所有写回都通过后端的 `ConfirmedAction`。

## 3. Lark CLI 优先原则

OAR 是基于 Lark CLI 开发的应用，但不能只是“调用 CLI”。业务层必须 **LarkAdapter 优先**，阶段 0.5 已确认 OKR 主路径可走 Lark CLI；生产仍需保留 OpenAPI 兜底路径。

核心技术原则：

> 把 Lark CLI 产品化成安全、可审计、可回滚的智能体工具层。

保守原则：

- 当前能力以本机安装版本的 schema 快照为准。
- 生产环境锁定 CLI 版本，不自动升级。
- 所有调用经过 `LarkAdapter`。
- 每个 command 都要有 allowlist、超时、输出长度限制、脱敏和审计。
- OpenAPI 兜底路径必须设计进架构，不能把 CLI 当成不可替换黑盒。

阶段 0.5 实测结果见：[`phase-0.5-lark-cli-validation-report.md`](phase-0.5-lark-cli-validation-report.md)。

## 4. LarkAdapter

所有飞书能力必须经过 `LarkAdapter`，业务代码不直接散落调用 CLI 或 OpenAPI。

| Adapter | 责任 |
| --- | --- |
| `OkrAdapter` | 读取周期、Objective、KR、progress、alignment；必要时写回低风险进度或评论 |
| `EvidenceAdapter` | 从 Docs、Tasks、Meetings、Minutes、IM、Calendar 收集证据 |
| `ActionAdapter` | 将建议动作转成任务、提醒、评论、会议草稿 |
| `AuthAdapter` | 管理 app/user/bot 身份、scope 检查、缺权限引导 |
| `AuditAdapter` | 记录 CLI/OpenAPI 调用、dry-run、确认、执行结果 |

最小接口：

```text
list_okr_cycles(user_id, user_id_type) -> OkrCycle[]
get_okr_cycle_detail(cycle_id) -> OkrCycleDetail
list_progress(target_id, target_type) -> ProgressRecord[]
dry_run_create_progress(request) -> ToolDryRun
create_progress(confirmed_action_id, request) -> ProgressRecord
dry_run_update_progress(request) -> ToolDryRun
update_progress(confirmed_action_id, request) -> ProgressRecord
```

所有写方法必须接收 `confirmed_action_id`，并写入 `AuditEvent`。

## 5. 账号、身份与 7x24

账号模型：

| 对象 | 作用 |
| --- | --- |
| `Tenant` | 飞书企业租户，数据隔离和计费边界 |
| `OarUser` | OAR 内部用户，保存偏好、角色、记忆设置 |
| `LarkIdentity` | 飞书用户身份绑定，如 `open_id`、`union_id`、租户信息 |
| `RoleBinding` | manager、PMO、admin、viewer 等 OAR 内角色 |
| `DeviceSession` | macOS / iOS / web / 飞书入口的登录设备和会话 |
| `TokenGrant` | 用户或企业授权给 OAR 的 OAuth grant，必须加密存储 |
| `AgentActor` | 智能体执行时使用的身份描述，不直接等同于某个 token |

执行身份：

| 身份 | 来源 | 适合场景 |
| --- | --- | --- |
| `user_delegated` | 用户 Lark OAuth 登录后授权 | 读取用户可见 OKR、日历、任务、文档；执行用户确认后的写回 |
| `bot_actor` | 飞书 Bot / 企业自建应用 | 发消息卡片、提醒、确认入口、系统通知 |
| `app_actor` | 企业应用授权 | 组织级同步、公开团队 OKR、后台批处理 |
| `service_actor` | OAR 后端内部身份 | 调度、队列、审计、模型任务 |
| `approved_user_action` | 用户确认后的动作 | 写评论、建任务、提醒 owner、更新低风险字段 |

关键原则：

- Lark 登录可以自动绑定用户身份，但不能自动放大成“智能体拥有该用户全部权限”。
- 实际可用权限必须取 Lark app scopes、用户授权、资源权限和 OAR 策略 allowlist 的交集。
- 如果需要 7x24 后台运行，需要申请 `offline_access`。
- `refresh_token` 只在用户授予 `offline_access` 时返回。
- 刷新成功后必须保存新的 refresh token，原 refresh token 可能失效。
- 客户端只保存 OAR session，不长期保存飞书 refresh token。

## 6. 多端同步

飞书与 OAR 的职责分界：

| 系统 | 作为权威数据源的数据 |
| --- | --- |
| 飞书 | OKR、Docs、Tasks、Meetings、Calendar、IM 原始数据 |
| OAR 后端 | 每周复盘、待确认动作、审计事件、证据索引、记忆、同步游标 |
| 客户端 | UI 缓存、草稿、设备会话，不保存长期 token |

同步原则：

- 所有客户端通过 `sync_cursor` 拉取增量状态。
- 写操作必须使用 `idempotency_key`。
- 后端维护 `OperationLedger`，记录待确认、已确认、执行中、已成功、已失败、已取消。
- 同一个待确认动作在 macOS / iOS / 飞书卡片中的状态必须一致。
- 客户端离线时只能编辑草稿，不能离线确认真实写回。

## 6.1 Phase 0.6 持久化草案

Phase 0.6 的首版 Postgres migration 草案位于：

[`../crates/oar-core/migrations/0001_phase_0_6_identity_action_audit.sql`](../crates/oar-core/migrations/0001_phase_0_6_identity_action_audit.sql)

覆盖对象：

| 表 | 责任 |
| --- | --- |
| `tenants` | 企业租户隔离边界 |
| `oar_users` | OAR 内部用户 |
| `lark_identities` | 飞书身份绑定 |
| `token_grants` | OAuth grant 元数据和加密授权包 |
| `device_sessions` | 多端会话和同步游标 |
| `confirmed_actions` | 用户确认后的动作 |
| `operation_ledger` | 幂等执行账本 |
| `audit_events` | append-only 审计事件 |
| `audit_outbox` | adapter 副作用与审计持久化之间的 crash-window 缓冲 |

关键约束：

- `confirmed_actions (tenant_id, idempotency_key)` 唯一。
- `operation_ledger (tenant_id, idempotency_key)` 唯一。
- `operation_ledger.action_id` 引用 `confirmed_actions.action_id`。
- `audit_events (trace_id, sequence)` 唯一。
- `audit_events` 有 `BEFORE UPDATE` / `BEFORE DELETE` trigger 阻止静默修改。
- `token_grants` 不使用明文 `access_token` / `refresh_token` 列名，授权材料保存为 `encrypted_oauth_grant`、`oauth_grant_key_id`、`oauth_grant_fingerprint`。

当前边界：

- 这是 schema contract 草案，不代表真实 Postgres 已接入运行时。
- Rust core 当前使用 repository trait + in-memory repository 验证语义。
- 下一步需要选择数据库访问层并实现 Postgres repository，验证 DB 事务、唯一约束、并发 upsert、outbox drain 和 crash recovery。

## 7. 智能体运行时与模型配置

OAR 的智能不应该来自一次 LLM 调用，而应该来自：

> 模型编排 + 可追溯证据 + 可学习的团队记忆 + 可版本化策略。

模型角色：

| 模型角色 | 用途 | 要求 |
| --- | --- | --- |
| `fast_model` | 分类、摘要、长期未更新检测、低风险初筛 | 低延迟、低成本、稳定 JSON |
| `reasoning_model` | 每周复盘、复杂风险判断、建议动作 | 推理强、上下文长、结构化输出 |
| `embedding_model` | 证据检索、记忆检索 | 向量质量稳定、可批量处理 |
| `local_or_private_model` | 高敏企业数据场景 | 可私有化、可审计、可按租户关闭外部模型 |

每次生成复盘或建议动作时记录：

- `prompt_version`
- `policy_version`
- `model_provider`
- `model_version`
- `tool_schema_version`
- `output_schema_version`
- `evidence_ids`
- `memory_ids`
- `trace_id`

记忆架构见：[`memory-architecture.md`](memory-architecture.md)。

## 8. 证据链

OAR 的每条智能体判断都必须绑定来源。

| 智能体判断 | 可用证据来源 |
| --- | --- |
| KR 长期未更新 | OKR 更新时间、owner 最近消息、任务更新时间 |
| KR 低于预期节奏 | OKR progress、任务完成率、会议/文档中的阻塞点 |
| owner 未响应 | IM 搜索、任务评论、OKR 评论、会议纪要 |
| 目标缺证据 | Docs/Wiki/Minutes/Tasks 中找不到关联上下文 |
| 需要跟进 | Minutes 待办、Calendar 空档、Tasks 未完成项 |
| 可以更新 progress | 已完成任务、会议结论、owner check-in、Base/Sheets 指标 |

MVP 原则：优先保存摘要、来源引用和 hash，不默认保存完整原文。
