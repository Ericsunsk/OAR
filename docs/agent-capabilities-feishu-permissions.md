# Agent capabilities 与飞书权限矩阵

更新日期：2026-05-29

本文定义 OAR 智能体能力、生产 adapter、`action_type`、飞书权限和执行门禁之间的对应关系。它是 `execution-audit.md` 的能力清单补充，不改变核心边界：

```text
ConfirmedAction -> OperationLedger -> PlatformAdapter -> AuditEvent
```

任何生产写回都必须经过这条链路。Lark CLI / Feishu CLI 只能用于本地验证、fixture 录制、parser 回归和 API 行为排查，不能作为生产写入路径。

## 1. 术语

| 名称 | 含义 |
| --- | --- |
| Agent capability | 用户可感知的智能体能力，例如读取 OKR、诊断风险、起草 progress、创建任务草稿 |
| PlatformAdapter | 生产调用边界，例如 `LarkAdapter`、`OkrAdapter`、`TaskAdapter`、`CalendarAdapter`、`MessageAdapter`、`AuthAdapter` |
| `ProposedActionKind` | Review Inbox 中待用户处理的建议动作类型，例如 `create_kr_progress` |
| `action_type` | OAR 执行策略和审计中的稳定动作键，例如 `okr.progress.update`；它不是飞书 scope |
| Feishu app scope | 飞书开放平台应用后台开通的权限，例如 `okr:okr.progress:writeonly` |
| OAuth grant scope | 用户扫码授权后实际进入 OAR `TokenGrant.scopes` 的权限；后台新增 app scope 后，旧 grant 不会自动增权 |
| OAR `required_scope` | OAR 内部策略键，可映射到一个或多个飞书 scope；例如 `okr.period.read`、`okr.content.read`、`okr.progress.write` |
| Capability execution mode | core 能力矩阵中的执行姿态：`AutoRead` 可自动读取，`DraftOnly` 只生成建议/草稿，`ConfirmedWrite` 才能进入生产写执行 allowlist |
| Agent tool manifest | 后端 Agent 工具声明，例如 `feishu.okr.summarize_my_okr`；只绑定 OAR `action_type` / capability，并由 core 矩阵派生 Feishu scopes，不直接授予平台权限 |

飞书 app scope、用户 OAuth grant scope 和 OAR allowlist 是三层不同门禁。读 OKR 必须先在飞书应用后台开通对应 read scope，再由用户通过 OAR 重新扫码把这些 scope 写入 `TokenGrant`；写回还必须额外满足 OAR policy allowlist、dry-run 和人工确认。旧 `TokenGrant` 不会因为后台新增 scope 自动扩大授权。

`ExecutionPolicy::from_capabilities(...)` 只把 `CapabilityExecutionMode = ConfirmedWrite` 且 `effect = Write` 的能力纳入外部写执行 allowlist。拥有 task、message、calendar 等写 scope 只代表可申请或可生成草稿，不代表可以直接生产写入。

## 2. 风险等级

| 等级 | 定义 | 默认门禁 |
| --- | --- | --- |
| R0 只读 | 读取当前用户或应用已授权可见的数据，不产生外部副作用 | 可自动执行；记录安全摘要和同步游标 |
| R1 内部派生 | 生成风险、摘要、草稿、排序、拒绝原因等 OAR 内部状态 | 可自动生成；进入待确认队列或内部审计 |
| R2 低影响单对象写入 | 单个 KR progress 创建或更新，不修改 owner、权重、周期、目标正文等 master fields | 必须 dry-run、人工确认、ledger 幂等、append-only audit |
| R3 协作型写入 | 发消息、写评论、建任务、约会议等会打扰他人或形成组织承诺的动作 | MVP 默认只生成草稿；启用前必须逐能力评审 |
| R4 高影响/破坏性写入 | 删除、批量写入、权限变更、OKR master fields、跨团队通知、管理员级操作 | MVP 禁止；需要单独设计、审批和回滚/补偿策略 |

## 3. 能力到权限矩阵

Core 能力矩阵的执行姿态合同：

| Execution mode | 用途 | 是否进入 `ExecutionPolicy` 写执行 allowlist |
| --- | --- | --- |
| `AutoRead` | 授权范围内的读取、同步、查询和安全摘要生成 | 否 |
| `DraftOnly` | 需要外部写 scope 才能最终落地的建议动作，但当前只生成草稿或待评审项 | 否 |
| `ConfirmedWrite` | 已完成 adapter、dry-run、人工确认、ledger 幂等和 audit 合同的单对象写入 | 是 |

| Agent capability | PlatformAdapter / operation | `ProposedActionKind` / `action_type` | Feishu scope 或权限要求 | 风险 | dry-run | 人工确认 | audit 要求 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 飞书 OAuth 登录与用户绑定 | `AuthAdapter.exchange_code` | 无业务 `ProposedAction`；auth audit 使用 `token_grant.*` / `token_refresh.*` | `offline_access` 用于 refresh token；如读取稳定 user id，可申请 `auth:user.id:read` | R1 | 不适用 | 用户在飞书 OAuth 同意页授权 | 记录 grant 生命周期、scope 摘要、actor、tenant，不记录 token/code |
| token refresh | `AuthAdapter.refresh` | `token_refresh.rotate`、`token_refresh.mark_needs_refresh`、`token_refresh.mark_reauth_required`、`token_refresh.mark_config_required`、`token_refresh.conflict_noop` | 原 grant 已包含 `offline_access` | R1 | 不适用 | 不需要每次人工确认；只维护授权材料 | 每次 refresh outcome 写 append-only `AuditEvent`，明文 token、fingerprint、encrypted blob 禁止入审计 |
| 读取 OKR 周期 | `OkrAdapter.list_cycles` | `action_type = okr.period.read`；`AutoRead` | `okr:okr.period:readonly` | R0 | 不适用 | 不需要 | 记录同步 job、目标租户、scope 摘要、游标和 safe error |
| 读取 Objective / KR 内容 | `OkrAdapter.list_cycle_objectives` / `list_objective_key_results` / `batch_get_okrs` | `action_type = okr.content.read`；`AutoRead` | `okr:okr.content:readonly` | R0 | 不适用 | 不需要 | 只保存摘要、引用、hash 和可见范围；不默认保存完整正文 |
| 读取 OKR progress | `OkrAdapter.list_progress` | `action_type = okr.progress.read`；`AutoRead` | `okr:okr.progress:readonly` | R0 | 不适用 | 不需要 | 记录来源引用、progress 摘要、content hash 和 parser 版本 |
| 读取 OKR review | 未来 `OkrAdapter.list_reviews` | `action_type = okr.review.read`；`AutoRead` | `okr:okr.review:readonly` | R0 | 不适用 | 不需要 | 记录 review 引用、可见范围、摘要 hash 和 parser 版本 |
| 读取 OKR setting | 未来 `OkrAdapter.get_settings` | `action_type = okr.setting.read`；`AutoRead` | `okr:okr.setting:read` | R0 | 不适用 | 不需要 | 记录设置来源、租户范围、字段摘要和同步版本 |
| 查询 calendar free-busy | 未来 `CalendarAdapter.get_free_busy` | `action_type = calendar.free_busy.read`；`AutoRead` | 首选 `calendar:calendar.free_busy:read`；若官方或租户返回 `calendar:calendar:readonly` 兼容能力，需用 fixture 记录后映射 | R0 | 不适用 | 不需要 | 记录查询时间窗、参与者安全摘要、来源 scope 和 safe error |
| 读取 task | 未来 `TaskAdapter.list_tasks` | `action_type = task.read`；`AutoRead` | `task:task:read` | R0 | 不适用 | 不需要 | 记录 task 引用、状态摘要、可见范围和同步游标 |
| 风险检测、周报、证据摘要 | `EvidenceAdapter` / risk engine | 无平台写入；内部可生成 `risk_detected` / `brief_generated` 审计事件 | 继承证据源读取 scope；不得扩大授权 | R1 | 不适用 | 不需要 | 记录输入 evidence refs、模型/规则版本、可见范围；不把模型输出当作证据本身 |
| 起草 KR progress 创建 | `OkrAdapter.dry_run_create_progress` | `create_kr_progress` / 目标 `action_type = okr.progress.create`；生产策略可先复用 `required_scope = okr.progress.write` | 飞书官方最小 scope：`okr:okr.progress:writeonly` | R2 | 必须；展示目标 KR、payload 摘要、before/after 和影响范围 | 必须确认或编辑后确认 | 成功、失败、拒绝、stale dry-run 都写 `AuditEvent`；审计只存安全摘要 |
| 起草 KR progress 更新 | `OkrAdapter.dry_run_update_progress` | `update_kr_progress` / `action_type = okr.progress.update`；当前 policy 测试使用 `required_scope = okr.progress.write` | 飞书官方最小 scope：`okr:okr.progress:writeonly` | R2 | 必须；执行前重新读取 live target 并校验 dry-run 指纹 | 必须确认或编辑后确认 | 记录 actor、scope、target、evidence、before/after 摘要、adapter operation id、结果 |
| progress 删除预览 | `OkrAdapter.dry_run_delete_progress` | `delete_kr_progress_dry_run` / 禁用 `action_type = okr.progress.delete` | 删除 scope：`okr:okr.progress:delete`；MVP 不申请或不启用执行 | R4 | 只允许 dry-run | 不进入真实执行 | 记录 dry-run 或 denied audit；禁止真实删除 |
| 修改 OKR 内容、owner、权重、周期、目标正文 | 未开放；未来只能走 `OkrAdapter` | 未来可能是 `okr.content.*` / `okr.period.*`；MVP 禁止 | 相关 scope 包括 `okr:okr.content:writeonly`、`okr:okr.period:writeonly` 或粗粒度 `okr:okr` | R4 | 不开放 | 不开放 | 若被请求，记录 denied audit 和原因 |
| 写 OKR/文档评论 | 未来 `ActionAdapter` / `CommentAdapter` | 未来 `comment.create`；MVP 仅草稿 | 文档评论可用 `docs:document.comment:create` 或 `docs:document.comment:write_only`；OKR 原生评论能力启用前需 API Explorer 复核 | R3 | 启用前必须展示接收位置、正文摘要和可见范围 | 必须 | 记录 comment target、正文 hash、引用证据和外部结果；不存完整敏感正文 |
| 提醒 owner / 发送飞书卡片 | 未来 `MessageAdapter.send` | `action_type = im.message.send`；core 当前为 `DraftOnly`，只生成消息草稿或待评审项 | bot 发送可申请 `im:message:send_as_bot`；用户身份发送/代发在启用前需 API Explorer/官方文档复核，不进入当前 allowlist | R3 | 生产启用前必须展示收件人、渠道、消息摘要、频控结果 | 生产启用前必须；系统状态通知需单独 allowlist | 草稿记录收件人类型、数量、消息 hash；生产启用后记录发送结果；禁止群发原始风险结论 |
| 创建任务 | 未来 `TaskAdapter.create_task` | `action_type = task.create`；core 当前为 `DraftOnly`，只生成任务草稿或待评审项 | 最小创建 scope：`task:task:writeonly`；完整任务管理 scope：`task:task:write` 不能因未来可能使用而提前进入 allowlist | R3 | 生产启用前必须展示 owner、标题、截止时间、来源证据 | 生产启用前必须 | 草稿记录 task target、assignee 摘要、payload hash；生产启用后记录外部结果 |
| 创建会议草稿 / 日程 | 未来 `CalendarAdapter.create_event` | 未来 `calendar.event.create`；MVP 仅草稿 | 创建日程需要日历写权限；官方权限键示例包括 `calendar:calendar`，读取忙闲为 `calendar:calendar:readonly` | R3 | 必须展示参会人、时间、日历、会议室、通知设置 | 必须 | 记录 event target、attendee 摘要、idempotency key、结果；避免默认发送通知 |
| 外部 A2A 提交建议 | A2A gateway -> OAR proposal service | 只能生成 `ProposedAction`，不能生成 `ConfirmedAction` | 不直接持有飞书 scope 或 token | R1-R3 | 平台写入前仍由 OAR adapter dry-run | 必须由 OAR 用户确认 | 记录外部 agent id、建议摘要、证据引用、后续用户决策 |

Agent tool manifest 与 core capability 的当前对齐：

| Agent tool | Required `action_type` | 派生 Feishu scope | Effect | 边界 |
| --- | --- | --- | --- | --- |
| `feishu.okr.summarize_my_okr` | `okr.period.read`、`okr.content.read` | `okr:okr.period:readonly`、`okr:okr.content:readonly` | Read | 只读汇总当前用户本人 OKR 周期、Objective 和 KR 数量；不读取团队/他人 OKR，不生成 `ProposedAction` 或 `ConfirmedAction` |

## 4. 生产执行规则

生产写入必须按以下顺序执行：

1. 读取 live platform state，生成证据引用、target fingerprint 和 payload hash。
2. 由 adapter 生成 dry-run。dry-run 必须包含目标对象、安全 payload 摘要、before/after 摘要、required scope 和风险等级。
3. 用户确认或编辑后确认，生成 `ConfirmedAction` 和 `idempotency_key`。编辑后确认必须保留原建议版本。
4. 写入或获取 `OperationLedger` 执行权。同一 `ActionID` / `idempotency_key` 只能产生一次外部副作用。
5. 执行前再次读取 live platform state。若 target fingerprint、scope、actor binding 或 payload hash 与确认时不一致，必须标记 stale 并要求重新确认。
6. 只通过 allowlist 中的 `PlatformAdapter` 调用飞书 OpenAPI。
7. 写入 terminal ledger 状态、append-only `AuditEvent` 和必要的 outbox 记录。

任何路径不得用 CLI stdout/stderr、LLM 工具调用结果或缓存记忆替代 live state。Memory 可以帮助排序和解释，不能作为物理写入证据。

## 5. Scope 与 allowlist 管理

- 未配置 `OAR_FEISHU_AUTH_SCOPE` 时，OAR 默认扫码登录请求已声明用户级能力所需的 Feishu scopes：`offline_access`、OKR 读写、OKR review/setting 读取、calendar free-busy、task 读写等。
- 默认 OAuth scope 由 core capability scope bundle 派生；OAuth grant 保存和校验飞书 scope 名称，执行策略使用 OAR `required_scope` / `action_type`。
- OAuth grant scope 不等于生产写执行 allowlist。只读 Agent tool runtime 使用 Feishu scope gate；写执行仍必须满足 `ExecutionPolicy` allowlist、dry-run、人工确认、`OperationLedger` 和 `AuditEvent`。
- 飞书开发者后台开启或新增 scope 后，用户必须重新用 OAR 扫码授权；旧 `TokenGrant.scopes` 不会自动增加新 scope。
- progress 创建/更新的 `okr:okr.progress:writeonly` 默认进入 OAuth grant，避免真实使用时反复补授权；生产执行仍只接受 `ConfirmedWrite` 能力、dry-run 和人工确认。
- 新增 scope 只能先作为 `AutoRead`、`DraftOnly` 或明确的 `ConfirmedWrite` 合同进入 core 矩阵；`task.create` 和 `im.message.send` 当前不进入生产执行 allowlist。
- `okr:okr.content:writeonly`、`okr:okr.period:writeonly`、`okr:okr`、`task:task:write`、`calendar:calendar`、`im:message` 等高权限或粗粒度 scope 不应因为“未来可能用到”提前进入生产 allowlist。
- OAR 内部 `required_scope` 必须有明确映射表。例如 `okr.progress.write` 映射到飞书 `okr:okr.progress:writeonly`，不能模糊映射到 `okr:okr`。
- 外部写执行 allowlist 只接受 `ConfirmedWrite` 能力。新增 scope、读能力或草稿能力时，必须显式确认 execution mode，避免 write scope 自动变成生产写权限。
- 新增 scope 前必须记录：目标 API、最小权限、actor kind、数据范围、风险等级、dry-run 展示内容、审计字段和回归 fixture。
- 如果飞书官方 scope 名称或 API 行为变化，先更新 fixture 和本文档，再开放生产执行。

## 6. 审计字段最低要求

每个 confirmed write 的 `AuditEvent` 至少包含：

| 字段 | 要求 |
| --- | --- |
| actor | OAR user、Lark identity、actor kind、确认时间 |
| scope | tenant、workspace、Feishu scope 摘要、OAR `required_scope` |
| target | resource type、resource id hash / safe hint、`action_type` |
| evidence | evidence refs、content hash、visibility scope |
| dry-run | target fingerprint、payload hash、before/after 摘要、risk level |
| ledger | `ActionID`、`idempotency_key`、operation id、terminal status |
| adapter result | safe status、safe error code、retry classification、external operation id |

审计中禁止出现 access token、refresh token、authorization code、full raw transcripts、raw CLI stdout/stderr、encrypted OAuth blob、fingerprint、完整会议原文或未脱敏敏感结论。

## 7. 当前启用结论

当前 MVP 只允许：

- 自动读取授权范围内的 OKR 周期、Objective、KR 和 progress。
- Agent 已启用 `feishu.okr.summarize_my_okr` 只读工具，用于当前用户本人 OKR 安全摘要；边界见上方 manifest 对齐表。
- 在 core capability 合同中将 OKR review、OKR setting、calendar free-busy 和 task 摘要列为 `AutoRead`；adapter 实现另行接入，且这些读能力不进入写执行 allowlist。
- 自动生成风险、证据摘要、周报和建议动作。
- 将 task 创建和 bot 消息发送登记为 `DraftOnly`；当前只允许生成草稿或待评审项合同，不进入生产写执行 allowlist。
- 对 KR progress 创建 / 更新进入受控写回验证路径。
- 对 progress 删除生成 dry-run 或 denied audit，但不执行。

当前 MVP 不允许：

- 自动创建、修改或删除 Objective / KR master fields。
- 自动删除 progress。
- 自动发群消息、建任务、约会议或写评论。
- 外部 A2A 智能体直接写回飞书。
- 通过 CLI、前端客户端或模型工具绕过 `ConfirmedAction -> OperationLedger -> PlatformAdapter -> AuditEvent`。

## 8. 官方资料

- 飞书 API 权限列表：https://open.feishu.cn/document/server-docs/application-scope/scope-list
- 飞书 OKR scope 列表页：https://open.feishu.cn/document/ukTMukTMukTM/uYTM5UjL2ETO14iNxkTN/scope-list?lang=zh-CN
- 创建日程 API：https://open.feishu.cn/document/server-docs/calendar-v4/calendar-event/create?lang=zh-CN
- 发送消息 API：https://open.feishu.cn/document/server-docs/im-v1/message/create?lang=zh-CN
