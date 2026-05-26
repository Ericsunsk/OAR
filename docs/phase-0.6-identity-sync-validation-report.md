# 阶段 0.6 身份与同步验证报告

更新日期：2026-05-26
执行时间：2026-05-26 10:28:49 +0800

## 1. 结论

阶段 0.6 已完成首轮身份、授权和刷新前置条件验证。
当前处于“安全契约已验证、编排回放已打通、真实客户端与调度待接入”的过渡状态，不应视为生产 refresh 闭环已完成。

三层状态（必须区分）：

1. 安全 parser / adapter contract（已部分通过）：定义并验证 refresh 输入输出安全边界与领域映射。
2. fixture replay -> Postgres orchestrator/UoW/audit（已部分通过）：用 fixture/fake adapter 打通事务化编排与审计留痕。
3. 真实 `AuthAdapter` client + scheduler（未完成）：真实 `lark-cli` / OpenAPI client 与后台调度尚未接入，不具备生产就绪声明条件。

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

以下事项必须等 OAR 后端落地并完成集成验证：

- `Tenant`、`OarUser`、`LarkIdentity` 的 Postgres repository 是否完成租户隔离、唯一约束与冲突语义验证。
- `TokenGrant` 是否仅以加密授权包持久化，并在 repository 边界禁止明文 token 进出。
- `TokenRefreshDecision` 是否通过 persistence bridge 安全映射为 CAS rotation、needs-refresh 或 reauth-required 持久化命令，并在 revoked / reauth-required grant 下阻断 refresh。
- Auth refresh parser 边界是否只接受加密授权包 envelope，并拒绝任何 plaintext token-like 输出后再进入 `RefreshOutcome` 映射。
- refresh token rotation 是否通过 SQL CAS 原子更新（`tenant_id + grant_id + expected_fingerprint + state guard`）。
- revoked / reauth-required grant 是否在 SQL 层直接阻断 rotation。
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
- `TokenGrant` repository 接口仅处理 `encrypted_oauth_grant`、`oauth_grant_key_id`、`oauth_grant_fingerprint`。
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
| I4 | 后端 `TokenGrant` 存储 | 进行中 | token 加密存储，refresh rotation 原子更新且受 SQL guard 约束 |
| I4a | identity repositories（`Tenant`/`OarUser`/`LarkIdentity`） | 进行中 | 租户隔离、绑定唯一性、冲突语义和审计字段在 Postgres 下可验证 |
| I4b | `TokenRefreshDecision` persistence bridge | 部分通过 | service 层已串起 refresh outcome、decision、repository command sink 和 allowlist 安全错误摘要；`PostgresTokenRefreshOrchestrator` 已验证 fake `AuthRefreshAdapter` -> domain decision -> transactional UoW -> append-only audit 的编排边界（live DB tests 覆盖 rotation success、stale conflict noop、transient failure redaction 和 revoked short-circuit）；真实 `AuthAdapter` 与后台 scheduler 尚未接入 |
| I4c | Auth refresh adapter contract / safe parser fixture boundary | 部分通过 | parser 仅接受加密授权包 envelope，检测到 access token / refresh token / authorization code 等 plaintext token-like 片段即拒绝；parser 输出已映射到领域 `RefreshOutcome`，并继续走 decision bridge 与审计脱敏路径；真实 `lark-cli` / OpenAPI refresh client 尚未接入 |
| I5 | 多端 `DeviceSession` 同步 | 进行中 | cursor 单调推进、stale/revoked 会话被拒绝且多端看到同一 action 状态 |
| I6 | `OperationLedger` 幂等执行 | 部分通过 | 同一 `ConfirmedAction` 并发确认只执行一次 |
| I7 | 后台 worker | 未开始 | 无客户端在线时仍可按计划生成复盘 |
| I8 | revoke / reauth | 未开始 | 授权撤销后停止执行并提示重新授权 |

## 5. 下一步

下一验证切片（进行中）：

1. `Tenant` / `OarUser` / `LarkIdentity` Postgres repositories 语义验证：`tenant_id` 隔离、identity 绑定唯一约束、冲突可恢复语义、最小审计字段落库。
2. `DeviceSession` Postgres repository 语义验证：`tenant_id` 隔离、`sync_cursor` 单调推进、revoked/expired 会话门禁、并发更新冲突信号。
3. 接入真实 `AuthAdapter` 与后台调度前，先补齐 refresh scheduler 前置能力：Postgres 租户级 due-candidate 安全筛选（`due` / `needs_refresh` / `expired`），并在查询层排除 `revoked` / `reauth_required` grant，确保候选快照不返回 `encrypted_oauth_grant` 或任何明文 token；CAS fingerprint 仅作为编排元数据使用，不写入日志或审计。
4. 将 `PostgresTokenRefreshOrchestrator` 接入真实 `AuthAdapter` 与后台调度，验证从 refresh attempt 到 Postgres CAS + audit 事务边界的生产路径。
5. 补齐真实 adapter / scheduler 路径下的审计集成验证：将 service report / audit summary 写入 append-only audit 事件，并确保不暴露 access token、refresh token、authorization code、raw CLI stdout/stderr、sink 内部错误、encrypted blob 或 fingerprint。
6. 验证 refresh 编排不越权：不直接暴露明文 token，不绕过 `LarkAdapter/AuthAdapter`，不触发未确认的 OKR 写回。
7. 以真实 adapter 输出回放 fixture，持续验证 safe parser 边界：只接受 encrypted envelope，拒绝 plaintext token-like 输出，再映射到 `RefreshOutcome`。

并行工作项：

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
- repository 层已支持 `TokenRefreshDecision` 通过 `PostgresTokenGrantRepository::apply_refresh_command` 分发到 `rotate_encrypted_grant`、`mark_refresh_failed`、`mark_reauth_required`，并复用现有 SQL CAS / state guard。
- 默认构建已加入 storage-agnostic `TokenRefreshService`，通过 `AuthRefreshAdapter` 与 `TokenRefreshCommandSink` 串联 refresh outcome、decision、repository command 和 allowlist 安全错误摘要；revoked / reauth-required / missing refresh material 会短路且不调用 adapter/sink。
- `postgres` feature 已加入 `PostgresTokenRefreshCommandSink`，将 storage-agnostic `TokenRefreshService` 接到 `PostgresTokenGrantRepository::apply_refresh_command`；live DB tests 已覆盖 rotation success、stale fingerprint conflict noop、report/audit debug redaction，以及 service error 输出不展开 sink 内部错误。
- 已加入 token refresh audit 映射边界：`TokenRefreshAuditSummary` 可映射为 append-only `AuditEvent`，复用现有 execution event types 并以 `target.resource_type = token_grant` / 稳定 `action_type` 区分 refresh 场景；默认测试覆盖 success、conflict noop、short-circuit 和 safe error redaction，Postgres live test 覆盖 audit roundtrip。
- 已验证 token refresh 场景下的 Postgres 事务化 UoW：refresh 状态更新与 audit append 可在同一 DB transaction 内提交，且审计写入失败会触发整体回滚；当前验证链路是 fake `AuthRefreshAdapter` -> service decision -> transactional UoW -> audit，仍限于 repository/UoW 与 live DB tests。
- 已加入 `PostgresTokenRefreshOrchestrator` 编排边界：短路路径不调用 adapter / UoW，只写 denied audit；可 refresh 路径调用 fake `AuthRefreshAdapter`、生成 domain decision，并经 transactional UoW 同事务写状态与 audit。live DB tests 覆盖 rotation success、stale conflict noop、transient failure redaction 和 revoked short-circuit。
- 已开始补齐 scheduler 前置查询能力：新增 Postgres 租户级 refresh due-candidate 选择语义（仅返回 `due` / `needs_refresh` / `expired` 候选），并在查询层排除 `revoked` 与 `reauth_required` grant；候选快照仅包含 grant id、tenant id、状态、refresh material 存在性和 CAS fingerprint 等最小必要元数据，不返回 `encrypted_oauth_grant` 或任何明文 token，fingerprint 不得进入日志或审计。

仍需生产级验证：

- `Tenant` / `OarUser` / `LarkIdentity` Postgres repositories 集成验证：租户隔离、唯一约束冲突路径、identity 绑定幂等恢复与审计可追溯。
- `TokenGrant` Postgres 持久化集成验证：repository 仅处理加密授权包，不接受/返回明文 token。
- `TokenRefreshService` 与 repository command sink 的领域编排已覆盖，refresh audit 事件映射、Postgres roundtrip 和 fake adapter 下的 transactional orchestrator 已验证；仍需接入真实 `AuthAdapter` 与后台调度，并在真实 adapter 路径下持续验证同一事务边界。
- Auth refresh adapter contract / safe parser fixture 边界已建立：当前仅在 fixture/fake adapter 下验证“加密 envelope -> `RefreshOutcome` -> decision bridge”链路；真实 `lark-cli` / OpenAPI client 输出尚未连通。
- 审计/日志边界必须持续成立：不得记录 access token、refresh token、authorization code、raw CLI stdout/stderr、encrypted blob 或 fingerprint。
- refresh rotation SQL CAS 集成验证：`tenant_id + grant_id + expected_fingerprint`、状态白名单（`valid` / `needs_refresh` / `expired`）和 `revoked_at IS NULL` / `reauth_required_at IS NULL` guard 全部生效。
- revoked / reauth-required grant 的 rotation 阻断需在真实数据库和并发场景下持续验证。
- `DeviceSession` Postgres repository 需补齐真实数据库并发验证：cursor 只前进不回退、revoked/expired 门禁、跨设备冲突可观测。
- `PostgresTokenRefreshOrchestrator` 与真实 adapter / scheduler 的生产集成验证需继续补齐：refresh 只经加密授权包与 CAS guard，且不绕过 `LarkAdapter/AuthAdapter`。
- token refresh background scheduler/daemon 仍未完成；当前仅补到“可安全选择候选 grant”的前置层，不代表已具备无人值守 refresh 执行能力。
- Postgres 级 `OperationLedger` 唯一约束 / upsert 的真实数据库验证需在提供 `DATABASE_URL` 的环境持续运行；多进程并发 race 仍需专门压力用例。
- Postgres executor / outbox worker 尚未接入真实后台调度、外部审计投递 sink 和 crash recovery。
- macOS、iOS、飞书卡片通过同一后端 repository 观察一致状态。
