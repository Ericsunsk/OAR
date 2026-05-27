# 阶段 0.6 身份与同步验证报告

更新日期：2026-05-27
执行时间：2026-05-27 00:00:00 +0800

## 1. 结论

阶段 0.6 已完成首轮身份、授权和刷新前置条件验证。
当前处于“安全契约已验证、编排回放已打通、Rust OpenAPI HTTP 装配已完成 fake transport 验证、live Feishu 与调度待接入”的过渡状态，不应视为生产 refresh 闭环已完成。

八层状态（必须区分）：

1. 安全 parser / adapter contract（已部分通过）：定义并验证 refresh 输入输出安全边界与领域映射。
2. fixture replay -> Postgres orchestrator/Recorder/audit（已部分通过）：用 fixture/fake adapter 打通事务化编排与审计留痕。
3. `AuthAdapter` safe transport client boundary（已部分通过）：core 已能消费上层 runtime transport 返回的 raw envelope，并在 auth 边界内完成大小上限、parser fail-closed 与安全错误分类；core 不执行 shell/CLI/OpenAPI。
4. 具体 Rust OpenAPI transport + scheduler（部分通过）：OAuth refresh 的 reqwest HTTP transport、授权材料解密提供器和生产装配入口已放入 `crates/oar-lark-adapter`，并通过 fake HTTP/transport 验证；真实 Feishu 网络调用、scheduler runtime 与生产监控仍未连通，不具备生产就绪声明条件。`lark-cli` 后续只作为本地验证与 fixture 录制工具，不作为生产主通道。
5. 候选筛选 + 单次 sweep（已部分通过）：已在候选选择与编排链路之间补充“显式触发的一次性 `run_once` sweep”，并加入 `DATABASE_URL`-gated Postgres live tests 覆盖逐 grant 调用既有 orchestrator/Recorder/audit、顺序审计和 `limit = 0` 短路；同时已补齐 durable Postgres scheduler lease primitive 作为 sweep 执行门禁，recurring 状态模型当前仅 `pending` / `running`。该能力不是 daemon，也不是无人值守后台循环。
6. tenant maintenance one-shot contract（已部分通过）：`oar-core` 维持 pure core/storage/contracts 边界，不实现 daemon/poll loop/HTTP/gRPC runtime；当前仅验证“显式触发一次的 tenant maintenance tick”可串联 lease-gated refresh scheduled sweep 与 audit outbox drain，并返回两段独立 stage report；scheduled sweep 硬错误不得跳过 outbox drain，stage failure 只暴露安全分类字符串，作为后续 runtime 常驻调度的前置契约。
7. tenant maintenance config fail-closed（已通过）：tenant maintenance 构造前已具备 `validate` / `try_new` 校验，`tenant_id` / `lease_id` / `audit_stream` / `scheduled_audit_trace_id` 为空或 `lease/retry/delay/limit/batch/max_attempts` 为 `0`/非正时会被拒绝；`0`/空值不再被接受为 noop 配置。
8. 依赖雷达边界（已对齐）：[`docs/reference/dependency-radar.md`](../reference/dependency-radar.md) 仅作为候选依赖技术雷达，不作为 Phase 0.6 生产采纳证明；`axum`、向量检索、文档解析、通用重试库当前不得进入 `oar-core` 或本阶段生产 refresh/maintenance 主路径。

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

- `Tenant`、`WorkspaceUser`、`LarkIdentity` 的 Postgres repository 是否完成租户隔离、唯一约束与冲突语义验证。
- `TokenGrant` 是否仅以加密授权包持久化，并在 repository 边界禁止明文 token 进出。
- `TokenRefreshDecision` 是否通过 persistence bridge 安全映射为 CAS rotation、needs-refresh 或 reauth-required 持久化命令，并在 revoked / reauth-required grant 下阻断 refresh。
- 飞书 refresh 配置缺失（如官方 `20074`）是否映射为 `refresh_config_required`，并暂停该 grant 的 due-candidate 重试，避免无限 transient sweep。
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

详细字段设计归入 [`technical-architecture.md`](../architecture/technical-architecture.md)。

## 4. 验证用例

| 测试 | 目标 | 当前状态 | 通过标准 |
| --- | --- | --- | --- |
| I0 | user / bot 身份验证 | 已通过 | `auth status --verify` 返回 user 和 bot verified |
| I1 | `offline_access` scope | 已通过 | `auth check --scope "offline_access"` 返回 ok |
| I2 | 用户身份读取 scope | 已通过 | `auth check --scope "auth:user.id:read"` 返回 ok |
| I3 | token refresh 前置条件 | 部分通过 | refresh 到期时间存在，token 从 needs_refresh 变为 valid |
| I4 | 后端 `TokenGrant` 存储 | 进行中 | token 加密存储，refresh rotation 原子更新且受 SQL guard 约束 |
| I4a | identity repositories（`Tenant`/`WorkspaceUser`/`LarkIdentity`） | 进行中 | 租户隔离、绑定唯一性、冲突语义和审计字段在 Postgres 下可验证 |
| I4b | `TokenRefreshDecision` persistence bridge | 部分通过 | service 层已串起 refresh outcome、decision、repository command sink 和 allowlist 安全错误摘要；`PostgresTokenRefreshOrchestrator` 已验证 fake `AuthRefreshAdapter` -> domain decision -> transactional Recorder -> append-only audit 的编排边界（live DB tests 覆盖 rotation success、stale conflict noop、transient failure redaction 和 revoked short-circuit）；Rust OpenAPI adapter 已完成 fake transport 装配验证，真实 Feishu 网络调用与后台 scheduler 尚未接入 |
| I4c | Auth refresh adapter contract / safe transport boundary | 部分通过 | parser 仅接受加密授权包 envelope，检测到 access token / refresh token / authorization code 等 plaintext token-like 片段即拒绝；`FeishuAuthRefreshSafeClient` 已把 runtime transport raw envelope 限制在 auth 边界内，加入响应大小上限、parser fail-closed 和安全错误分类；parser 输出已映射到领域 `RefreshOutcome`，并继续走 decision bridge 与审计脱敏路径；Rust 原生 OpenAPI transport、授权材料解密提供器已在 adapter crate 完成 fake transport 装配验证，后台 scheduler 尚未接入（`lark-cli` 仅用于本地验证与 fixture） |
| I4d | Rust OpenAPI HTTP 装配边界（adapter crate） | 部分通过 | `crates/oar-lark-adapter/tests/postgres_refresh_integration.rs` 已用 fake HTTP/transport 覆盖 endpoint、method、headers、body schema、`max_response_bytes` 传递、5xx/oversized transport error -> transient safe error、`20074`/`20037` 分类映射、material missing/fingerprint mismatch/malformed blob/wrong key -> safe transient/no HTTP/no rotate，以及 debug/audit secret redaction；当前仍未连接真实 Feishu 网络，不能宣称 live OpenAPI 验证完成 |
| I5 | 多端 `DeviceSession` 同步 | 进行中 | cursor 单调推进、stale/revoked 会话被拒绝且多端看到同一 action 状态 |
| I6 | `OperationLedger` 幂等执行 | 部分通过 | 同一 `ConfirmedAction` 并发确认只执行一次 |
| I7 | 后台 worker | 未开始 | 无客户端在线时仍可按计划生成复盘 |
| I8 | revoke / reauth | 未开始 | 授权撤销后停止执行并提示重新授权 |
| I9 | 单次 refresh sweep（`run_once`） | 部分通过 | 候选按 `tenant_id` 选择后，可由显式触发的一次性 sweep 逐 grant 进入既有 orchestrator/Recorder/audit；sweep 受 durable Postgres scheduler lease gate 约束，recurring 状态当前仅 `pending` / `running`；已加入 `DATABASE_URL`-gated live tests 覆盖成功批次、顺序审计、租户/到期过滤继承和 `limit = 0` 不调用 adapter / 不写 audit；不宣称后台 daemon 或连续调度能力 |
| I10 | tenant maintenance one-shot tick（refresh sweep + outbox drain） | 部分通过 | core 仅暴露显式触发的一次性租户维护 tick 契约，不实现 daemon/poll loop/HTTP/gRPC runtime；tick 返回 `scheduled_sweep` / `outbox_drain` 两段 stage report，scheduled sweep 硬错误不跳过 outbox drain，stage failure 仅暴露安全分类字符串；且保持 refresh 只经 `AuthAdapter`、OKR 写回只经 `ConfirmedAction`、审计/日志不暴露 raw token、raw CLI stdout/stderr、encrypted blob 或 fingerprint；真实 auth transport、常驻 scheduler 与生产监控仍未接入 |
| I11 | tenant maintenance config fail-closed 校验 | 已通过 | `validate/try_new` 拒绝空 `tenant_id/lease_id/audit_stream/trace_id` 与非正 `lease/retry/delay/limit/batch/max_attempts`；`0`/空值不作为 noop |
| I12 | 依赖技术雷达分层边界 | 已通过 | `docs/reference/dependency-radar.md` 仅作候选雷达；`axum`/向量/文档解析/重试库不进入 `oar-core` 与当前 Phase 0.6 生产路径 |

## 5. 下一步

下一验证切片（进行中）：

1. `Tenant` / `WorkspaceUser` / `LarkIdentity` Postgres repositories 语义验证：`tenant_id` 隔离、identity 绑定唯一约束、冲突可恢复语义、最小审计字段落库。
2. `DeviceSession` Postgres repository 语义验证：`tenant_id` 隔离、`sync_cursor` 单调推进、revoked/expired 会话门禁、并发更新冲突信号。
3. 接入真实 `AuthAdapter` 与后台调度前，持续维护 refresh scheduler 前置能力：Postgres 租户级 due-candidate 安全筛选（`due` / `needs_refresh` / `expired`），并在查询层排除 `revoked` / `reauth_required` / `refresh_config_required` grant，确保候选快照不返回 `encrypted_oauth_grant` 或任何明文 token；CAS fingerprint 仅作为编排元数据使用，不写入日志或审计。
4. 在已验证的单次 sweep / `run_once` 之上补齐 scheduler 触发前契约：基于已落地 lease gate、per-attempt audit sequence window 与 recurring `pending` / `running` 状态模型，继续完善失败中断/后续重试语义、租户粒度触发入口，以及真实 adapter 接入前的安全边界验证；继续明确它不是常驻 daemon，也不将整轮 sweep 包装为单个大事务。
5. 在 tenant maintenance one-shot tick 契约上补齐 runtime 接口：定义 runtime 侧的周期触发/cancellation 语义（例如 Tokio interval + cancellation token），但不把循环语义下沉进 `oar-core`。
6. 将 `PostgresTokenRefreshOrchestrator` 接入 Rust 原生 OpenAPI auth transport 与后台调度，验证从 refresh attempt 到 Postgres CAS + audit 事务边界的生产路径；当前不引入跨语言 SDK bridge。
7. 补齐真实 adapter / scheduler 路径下的审计集成验证：将 service report / audit summary 写入 append-only audit 事件，并确保不暴露 access token、refresh token、authorization code、raw CLI stdout/stderr、sink 内部错误、encrypted blob 或 fingerprint。
8. 验证 refresh 编排不越权：不直接暴露明文 token，不绕过 `LarkAdapter/AuthAdapter`，不触发未确认的 OKR 写回。
9. 以真实 adapter transport 输出回放 fixture，持续验证 safe transport + parser 边界：只接受 encrypted envelope，拒绝 plaintext token-like 输出，再映射到 `RefreshOutcome`。

并行工作项：

1. 接入 Rust 原生 OpenAPI auth transport 与授权材料解密提供器，打通真实 refresh 输出到 `RefreshOutcome` 的安全解析与编排链路；继续用 `lark-cli` 录制 fixture 和做本地验证。
2. 接入后台 scheduler/daemon，补齐无人值守 refresh 与失败重试的触发语义验证。
3. 补齐多端真实联调（macOS / iOS / 飞书卡片）下 `DeviceSession` 一致性与冲突恢复验证。
4. 在真实 adapter + scheduler 路径下持续验证审计脱敏与 append-only 落库边界。

## 6. 工程实现进展

本节更新日期：2026-05-27

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
- 已加入 Postgres `PostgresExecutionRecorder` storage 边界，可在一个 DB transaction 内提交 ledger + audit + outbox，并通过 live tests 验证 commit 与 audit append 失败回滚。
- 已加入 feature-gated async `PostgresActionExecutor`，用 Postgres Recorder 串起确认记录、dry-run、adapter execute、终态 ledger、audit event 和 outbox；live tests 覆盖成功、重复幂等、adapter failure 和 policy denied。
- 已加入 feature-gated `PostgresAuditOutboxWorker` 最小 drain 路径，outbox mark sent/retry/failed 支持 `attempt_count + lease_until` guard；dispatcher error 仍按 retryable 处理，但达到 `max_attempts` 后转入 `failed` 并在 report 中计为 `exhausted`，作为本地 poison-message 隔离边界；live tests 覆盖 lease 过期后二次 claim 时陈旧 worker 不能误标 sent、同一 claim 只能终态一次、retry 后重新 claim 的 attempt 单调递增、sent/retry/failed 混合投递，以及 retry cap 前后状态。
- audit outbox payload 安全边界正在收紧：目标是仅允许最小事件引用 envelope 入库，并在 repository/Recorder 边界拒绝 token、authorization、raw stdout/stderr、encrypted blob、fingerprint 等敏感字段或值。
- Postgres ledger submit 已改为显式返回 `created` 标记，避免用 `operation_id` 推断新建/复用；未确认 action 会在 DB 写入前被拒绝。
- repository 层已支持 `TokenRefreshDecision` 通过 `PostgresTokenGrantRepository::apply_refresh_command` 分发到 `rotate_encrypted_grant`、`mark_refresh_failed`、`mark_reauth_required`，并复用现有 SQL CAS / state guard。
- 默认构建已加入 storage-agnostic `TokenRefreshService`，通过 `AuthRefreshAdapter` 与 `TokenRefreshCommandSink` 串联 refresh outcome、decision、repository command 和 allowlist 安全错误摘要；revoked / reauth-required / missing refresh material 会短路且不调用 adapter/sink。
- `postgres` feature 已移除早期同步 `PostgresTokenRefreshCommandSink` runtime bridge；Postgres refresh 写入改由 async `PostgresTokenRefreshOrchestrator` / `PostgresTokenRefreshRecorder` 直接执行，避免在 async 环境中额外创建 runtime/thread。live DB tests 已覆盖 rotation success、stale fingerprint conflict noop、report/audit debug redaction。
- 已加入 token refresh audit 映射边界：`TokenRefreshAuditSummary` 可映射为 append-only `AuditEvent`，复用现有 execution event types 并以 `target.resource_type = token_grant` / 稳定 `action_type` 区分 refresh 场景；默认测试覆盖 success、conflict noop、short-circuit 和 safe error redaction，Postgres live test 覆盖 audit roundtrip。
- Postgres audit trace 已收紧为租户级唯一与租户级读取：`audit_events (tenant_id, trace_id, sequence)` 允许不同租户复用同一 trace/sequence 而不冲突，repository 查询必须携带 `tenant_id + trace_id`；live tests 覆盖跨租户同 trace 不串读。
- 已验证 token refresh 场景下的 Postgres 事务化 Recorder：refresh 状态更新与 audit append 可在同一 DB transaction 内提交，且审计写入失败会触发整体回滚；Recorder 入口已收敛为 `TokenRefreshPlannedCommand`，并在写入前校验 command/report 的 tenant、grant 和 command kind 一致性；当前验证链路是 fake `AuthRefreshAdapter` -> planned command/report -> transactional Recorder -> audit，仍限于 repository/Recorder 与 live DB tests。
- 已加入 `PostgresTokenRefreshOrchestrator` 编排边界：短路路径不调用 adapter / Recorder，只写 denied audit；可 refresh 路径调用 fake `AuthRefreshAdapter`、生成 domain decision，并经 transactional Recorder 同事务写状态与 audit。live DB tests 覆盖 rotation success、stale conflict noop、transient failure redaction 和 revoked short-circuit。
- 已补齐 scheduler 前置查询能力：新增 Postgres 租户级 refresh due-candidate 选择语义（仅返回 `due` / `needs_refresh` / `expired` 候选），并在查询层排除 `revoked` 与 `reauth_required` grant；候选快照仅包含 grant id、tenant id、状态、refresh material 存在性和 CAS fingerprint 等最小必要元数据，不返回 `encrypted_oauth_grant` 或任何明文 token，fingerprint 不得进入日志或审计。
- 已在候选筛选与 orchestrator 之间增加单次 refresh sweep（`run_once`）切片：显式触发后对候选逐 grant 调用既有 `PostgresTokenRefreshOrchestrator`，沿用每 grant 的 Recorder/audit 事务语义；sweep 前置为 durable Postgres scheduler lease gate，并持久化 recurring `pending` / `running` 状态；scheduled sweep report 保留 acquisition detail，审计序列按 lease attempt 分配独立 window，降低重试/抢租约后的 `(trace_id, sequence)` 碰撞风险；backlog-aware 调度使用 `limit + 1` 仅探测 `has_more`，只处理前 `limit` 个 grant，剩余候选触发短 `backlog_next_run_delay_ms` 重排；已加入 `DATABASE_URL`-gated live tests 覆盖成功批次、顺序审计、候选过滤继承、`limit = 0` 不调用 adapter 且不写 audit；当前不是后台 daemon，不提供无人值守循环能力，也不把整轮 sweep 作为单个跨 grant 大事务。
- 已将“tenant maintenance one-shot tick”明确为 runtime 前置契约：当前仅承诺 core 可被显式触发执行一次租户维护，串联 lease-gated refresh scheduled sweep 与 audit outbox drain，并返回两段独立 stage report；scheduled sweep 硬错误不再跳过 outbox drain，stage failure 只暴露安全分类字符串；`oar-core` 不承诺也不实现 daemon/poll loop/HTTP/gRPC runtime，常驻调度由后续 runtime 层负责。
- `token_refresh` / `auth` 已迁移到真实子模块路径并移除 root facade 兼容层，refresh 编排不再经根层转发入口。
- 已加入 `FeishuAuthRefreshSafeClient` safe transport boundary：core 可以消费上层 runtime transport 返回的 raw envelope，但仅在 auth 边界内做响应大小检查、safe parser fail-closed 和安全错误分类；Debug/Display 不暴露 raw stdout/stderr、plaintext token-like 内容、fingerprint 或 encrypted bytes；core 仍不实现 shell/CLI/OpenAPI transport。官方 `20074` 等配置类错误应进入 `refresh_config_required`，并暂停 due-candidate 重试。
- 已将 Postgres refresh 编排边界升级为 async adapter：`PostgresTokenRefreshOrchestrator` / sweep / tenant maintenance 可等待异步 Feishu adapter，而不要求生产 refresh 在 async runtime 中执行 blocking HTTP/DB。`oar-lark-adapter` 已补 Rust 原生 async reqwest transport、AEAD stored grant material provider、feature-gated Postgres grant material store、独立 app credential provider，以及“stored encrypted blob -> decrypt renewal -> fake Feishu HTTP success -> safe parser -> core async adapter -> RefreshOutcome::Success”的单元闭环；material provider 失败会 fail-closed 为 safe transient，且不会发起 Feishu HTTP。

仍需生产级验证：

- `Tenant` / `WorkspaceUser` / `LarkIdentity` Postgres repositories 集成验证：租户隔离、唯一约束冲突路径、identity 绑定幂等恢复与审计可追溯。
- `TokenGrant` Postgres 持久化集成验证：repository 仅处理加密授权包，不接受/返回明文 token。
- `TokenRefreshService` 与 repository command sink 的领域编排已覆盖，refresh audit 事件映射、Postgres roundtrip 和 fake adapter 下的 transactional orchestrator 已验证；仍需接入真实 Feishu 网络调用与后台调度，并在真实 adapter 路径下持续验证同一事务边界。
- Auth refresh adapter contract / safe transport 边界已建立：当前已验证“runtime transport raw envelope -> size guard -> safe parser -> `RefreshOutcome` -> decision bridge”链路，并已用 fake HTTP 验证 Rust OpenAPI adapter 装配闭环；adapter crate 侧新增 `DATABASE_URL` gated 集成测试，覆盖 Postgres stored encrypted grant material 进入 Rust OpenAPI auth adapter，再回到 core Postgres CAS/audit 的成功、`20074` config-required、`20037` reauth-required、5xx transient，以及 missing row / stale fingerprint / malformed blob / wrong key material failure 链路，并验证 `refresh_config_required` 会暂停 due-candidate 重试，material failure 不发起 HTTP、不 rotate 且审计脱敏。真实 Feishu 网络调用、scheduler runtime 与生产监控仍未连通。
- 真实 HTTP 生产装配范围已限定在 `crates/oar-lark-adapter`：endpoint、headers/body schema、response size/status/error 映射与 secret redaction 只在 adapter crate 组装和验证；`oar-core` 不引入 HTTP 运行时、请求拼装或 SDK sidecar。
- 审计/日志边界必须持续成立：不得记录 access token、refresh token、authorization code、raw CLI stdout/stderr、encrypted blob 或 fingerprint。
- refresh rotation SQL CAS 集成验证：`tenant_id + grant_id + expected_fingerprint`、状态白名单（`valid` / `needs_refresh` / `expired`）和 `revoked_at IS NULL` / `reauth_required_at IS NULL` guard 全部生效。
- revoked / reauth-required grant 的 rotation 阻断需在真实数据库和并发场景下持续验证。
- `DeviceSession` Postgres repository 需补齐真实数据库并发验证：cursor 只前进不回退、revoked/expired 门禁、跨设备冲突可观测。
- `PostgresTokenRefreshOrchestrator` 与具体 auth transport / scheduler 的生产集成验证需继续补齐：refresh 只经加密授权包与 CAS guard，且不绕过 `LarkAdapter/AuthAdapter`；不引入跨语言 SDK bridge。
- token refresh background scheduler/daemon 仍未完成；当前仅补到“可安全选择候选 grant + lease-gated 显式单次 `run_once` sweep + recurring `pending` / `running` 持久化基础”切片，不代表已具备无人值守 refresh 执行能力。
- Postgres 级 `OperationLedger` 唯一约束 / upsert 的真实数据库验证需在提供 `DATABASE_URL` 的环境持续运行；多进程并发 race 仍需专门压力用例。
- Postgres executor / outbox worker 尚未接入真实后台调度、外部审计投递 sink、failed outbox 运维恢复入口和 crash recovery。
- tenant maintenance 仍停留在 one-shot contract：具体 Rust OpenAPI auth transport、常驻 scheduler、runtime cancellation/重试策略、stage-level alerting 与生产监控闭环仍待完成。
- macOS、iOS、飞书卡片通过同一后端 repository 观察一致状态。
