# 阶段 0.6 身份与同步验证报告

更新日期：2026-05-26
执行时间：2026-05-26 10:28:49 +0800

## 1. 结论

阶段 0.6 已完成首轮身份、授权和刷新前置条件验证。

当前判断：

> 身份与 refresh 前置条件成立，但生产级 `TokenGrant` 存储、refresh token rotation、多端同步和幂等执行仍需后端实现后验证。

已验证：

- `lark-cli auth status --verify` 可验证当前 user 和 bot 身份。
- 当前默认执行身份为 user。
- user token 当前状态为 valid。
- user 身份和 bot 身份均可被服务端验证。
- `offline_access` 已授权。
- `auth:user.id:read` 已授权。
- access token 和 refresh token 均有明确到期时间。
- CLI 未输出 access token 或 refresh token 明文。

## 2. 不能仅靠 CLI 证明的事项

以下事项必须等 OAR 后端实现后验证：

- `TokenGrant` 是否加密存储。
- refresh token rotation 后是否原子保存新的 refresh token。
- token revoke 后后台 worker 是否停止执行。
- 同一个 `ConfirmedAction` 是否只执行一次。
- 多端同时确认时是否只产生一个 `OperationLedger` 执行记录。
- macOS、iOS、飞书卡片是否通过同一后端状态机看到一致状态。
- 定时 worker 是否能在无人打开客户端时按计划运行。
- 审计事件是否完整记录 actor、scope、target、before/after 和执行结果。

## 3. 阶段 0.6 模型边界

阶段 0.6 需要实现并验证四类对象：

| 对象 | 责任 |
| --- | --- |
| `TokenGrant` | 保存用户或企业授权，负责加密、refresh、revoke 和 reauth 状态 |
| `DeviceSession` | 表示 macOS / iOS / Web / 飞书入口会话，持有 OAR session 和 `sync_cursor` |
| `OperationLedger` | 记录待确认动作的状态机，保证同一 `ConfirmedAction` 只执行一次 |
| `AuditEvent` | append-only 记录 actor、scope、target、before/after 和执行结果 |

关键原则：

- 客户端只保存 OAR session，不保存飞书 refresh token。
- `TokenGrant` 不向智能体运行时暴露明文 token。
- 状态转移必须由后端原子完成。
- `idempotency_key` 必须唯一。
- 所有工具执行结果必须写入 `AuditEvent`。

详细字段设计归入 [`technical-architecture.md`](technical-architecture.md)。

## 4. 验证用例

| 测试 | 目标 | 当前状态 | 通过标准 |
| --- | --- | --- | --- |
| I0 | user / bot 身份验证 | 已通过 | `auth status --verify` 返回 user 和 bot verified |
| I1 | `offline_access` scope | 已通过 | `auth check --scope "offline_access"` 返回 ok |
| I2 | 用户身份读取 scope | 已通过 | `auth check --scope "auth:user.id:read"` 返回 ok |
| I3 | token refresh 前置条件 | 部分通过 | refresh 到期时间存在，token 从 needs_refresh 变为 valid |
| I4 | 后端 `TokenGrant` 存储 | 未开始 | token 加密存储，refresh rotation 原子更新 |
| I5 | 多端 `DeviceSession` 同步 | 未开始 | 多端看到同一 action 状态 |
| I6 | `OperationLedger` 幂等执行 | 部分通过 | 同一 `ConfirmedAction` 并发确认只执行一次 |
| I7 | 后台 worker | 未开始 | 无客户端在线时仍可按计划生成复盘 |
| I8 | revoke / reauth | 未开始 | 授权撤销后停止执行并提示重新授权 |

## 5. 下一步

1. 定义 `TokenGrant`、`DeviceSession`、`OperationLedger`、`AuditEvent` schema。
2. 将阶段 0.5 的 OKR CLI 输出保存为 `LarkAdapter` fixture。
3. 实现 `ConfirmedAction -> OperationLedger -> LarkAdapter -> AuditEvent` 的最小状态机。
4. 用本地并发测试模拟两个客户端同时确认同一动作，验证只写回一次。

## 6. 工程实现进展

更新日期：2026-05-26

本地 Rust 核心已完成以下 Phase 0.6 骨架验证：

- `TokenGrant` 生命周期已覆盖 refresh success、refresh missing token、revoke block refresh、debug redaction。
- `DeviceSession` 同步模型已覆盖 cursor 前进、stale cursor 拒绝、revoke 后阻止同步。
- `ExecutionPolicy` 已接入 `ActionExecutor` 的写入前置门禁；policy denied 不创建 ledger、不调用 adapter，并写入独立 `ExecutionDenied` 审计事件。
- `OperationLedger` 已覆盖重复幂等、并发重复提交、状态转移、未知 idempotency key 错误一致性。
- `AuditEventRepository` 和 `OperationLedgerRepository` 已形成内存实现和 repository 边界，执行器可通过 repository 读写 ledger/audit。
- Postgres migration 草案已加入 `crates/oar-core/migrations/0001_phase_0_6_identity_action_audit.sql`，并通过 schema contract 测试覆盖关键唯一约束、append-only 审计、token 字段命名安全和 sync cursor 字段。
- `storage::postgres` 已加入 SQL contract，覆盖 confirmed action / ledger 幂等 upsert、状态转移 guard、audit append、outbox enqueue 和 outbox claim/sent/retry/failed 状态更新。
- `postgres` / `postgres-sqlx` feature 已加入可选 `sqlx` repository 类型和 async 方法，已完成编译级验证。
- 已加入 `DATABASE_URL` gated live Postgres repository tests，可在本机或 CI 提供 Postgres 时验证 migration bootstrap、tenant-scoped ledger lookup、幂等状态转移、audit append-only trigger、outbox enqueue 默认值和 outbox claim/mark 状态机；未提供 `DATABASE_URL` 时默认跳过。
- 已加入 Postgres `PostgresExecutionUnitOfWork` storage 边界，可在一个 DB transaction 内提交 ledger + audit + outbox，并通过 live tests 验证 commit 与 audit append 失败回滚。
- 已加入 feature-gated async `PostgresActionExecutor`，用 Postgres UoW 串起确认记录、dry-run、adapter execute、终态 ledger、audit event 和 outbox；live tests 覆盖成功、重复幂等、adapter failure 和 policy denied。
- 已加入 feature-gated `PostgresAuditOutboxWorker` 最小 drain 路径，outbox mark sent/retry/failed 支持 `attempt_count + lease_until` guard；live tests 覆盖 lease 过期后二次 claim 时陈旧 worker 不能误标 sent、同一 claim 只能终态一次、retry 后重新 claim 的 attempt 单调递增，以及 sent/retry/failed 混合投递。
- Postgres ledger submit 已改为显式返回 `created` 标记，避免用 `operation_id` 推断新建/复用；未确认 action 会在 DB 写入前被拒绝。

仍需生产级验证：

- `TokenGrant` 加密持久化和 refresh token rotation 的数据库事务。
- Postgres 级 `OperationLedger` 唯一约束 / upsert 的真实数据库验证需在提供 `DATABASE_URL` 的环境持续运行；多进程并发 race 仍需专门压力用例。
- Postgres executor / outbox worker 尚未接入真实后台调度、外部审计投递 sink 和 crash recovery。
- macOS、iOS、飞书卡片通过同一后端 repository 观察一致状态。
