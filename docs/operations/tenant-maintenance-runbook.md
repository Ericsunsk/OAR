# Tenant Maintenance Recovery Runbook

更新日期：2026-05-31

本 runbook 覆盖 Phase 0.6 tenant maintenance 的排障和恢复规划。除已实现的确认后单 grant auth refresh resume 外，所有后续 requeue、reset 或外部平台写回都必须另走 `ConfirmedAction -> OperationLedger -> PlatformAdapter -> AuditEvent` 或明确的 auth lifecycle 审计路径。

## 1. Provider Revoke 边界

截至 2026-05-31，当前 Feishu 认证授权文档列出的 OAuth 用户令牌能力包括：

- `POST /open-apis/authen/v2/oauth/token`：获取 / 刷新 `user_access_token`。
- `POST /open-apis/passport/v1/sessions/logout`：退出用户登录态，需要 `tenant_access_token` 与 `passport:session:logout`。

`passport/v1/sessions/logout` 管理的是飞书登录态，不等同于撤销 OAR 持有的 OAuth grant。当前没有接入官方 OAuth grant revoke endpoint；OAR 的 logout grant revoke 语义是本地 `token_grants.state = revoked`，并写入 append-only audit。除非官方文档确认存在 OAuth grant revoke API，否则不要把登录态 logout 包装成 provider grant revoke。

## 2. 只读恢复报告

`PostgresOperationalRecoveryRepository::load_tenant_recovery_report(tenant_id, limit)` 返回 tenant-scoped 只读报告：

- failed `audit_outbox`：只返回 `id`、`stream`、`aggregate_id`、`attempt_count`、`created_at_ms` 和经过 `SafeAuditOutboxPayload` 验证的 payload 摘要；如果历史 payload 不安全，只返回 `payload_safe = false`，不回显 payload。
- parked `token_grants`：只返回 grant id、identity id、actor/scope boundary、state、安全错误、刷新/reauth/更新时间，以及推荐动作；不返回 `encrypted_oauth_grant`、key id、fingerprint、access token、refresh token 或 provider 原始响应。

当前推荐动作仅用于人工判断：

| recovery action | 含义 | 下一步 |
| --- | --- | --- |
| `InspectFailedAuditOutbox` | 审计 outbox 已进入 `failed` | 人工检查 sink 配置与外部审计系统；未来需走确认后的 requeue 动作 |
| `FixFeishuRefreshConfigThenResume` | grant 被 `refresh_config_required` / parser / oversized sentinel 暂停 | 修复 Feishu app/OAuth 配置；可走确认后的单 grant resume 动作清除 sentinel |
| `AskUserToReauthorize` | grant 已进入 `reauth_required` | 引导用户重新扫码授权，正常登录持久化会覆盖旧 grant |

## 3. 确认后 Resume

`PostgresOperationalRecoveryRepository::execute_confirmed_recovery` 当前只开放 `ResumePausedAuthRefresh`：

- 必须传入 confirmed `ConfirmedAction`、`operation_id`、`audit_trace_id`、单个 `grant_id` 和 `expected_updated_at_ms`。
- 执行会在事务内重新读取 live row、记录 dry-run audit，再只清除 `last_refresh_error`；不改 grant state、encrypted material、key id、fingerprint、revoked 或 reauth 字段。
- WHERE guard 只允许 `refresh_config_required`、`auth_refresh_parse_failed`、`auth_refresh_oversized_response`，且要求 `state IN ('valid', 'needs_refresh', 'expired')`、未 revoked、未 reauth、仍有 refresh material。
- success / stale no-op 都会绑定 `operation_id` 写入 append-only `AuditEvent` 和 `audit_outbox`；同一 idempotency key 重放不会二次恢复或二次写 audit/outbox。

## 4. 操作禁区

- 不要直接修改 `audit_outbox.status` 从 `failed` 回 `pending`。
- 不要直接清空 `token_grants.last_refresh_error`。
- 不要直接把 `reauth_required` 改回 `valid` 或 `needs_refresh`。
- 不要在日志、工单或聊天里粘贴 raw token、authorization code、encrypted blob、fingerprint、provider 原始响应或完整 outbox payload。

下一步可在 admin/operator auth 边界明确后新增只读 HTTP route；failed outbox requeue 写入口仍需单独设计 dry-run、确认、幂等和审计。
