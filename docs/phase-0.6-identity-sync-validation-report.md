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
| I6 | `OperationLedger` 幂等执行 | 未开始 | 同一 `ConfirmedAction` 并发确认只执行一次 |
| I7 | 后台 worker | 未开始 | 无客户端在线时仍可按计划生成复盘 |
| I8 | revoke / reauth | 未开始 | 授权撤销后停止执行并提示重新授权 |

## 5. 下一步

1. 定义 `TokenGrant`、`DeviceSession`、`OperationLedger`、`AuditEvent` schema。
2. 将阶段 0.5 的 OKR CLI 输出保存为 `LarkAdapter` fixture。
3. 实现 `ConfirmedAction -> OperationLedger -> LarkAdapter -> AuditEvent` 的最小状态机。
4. 用本地并发测试模拟两个客户端同时确认同一动作，验证只写回一次。
