# 安全、权限与执行边界

更新日期：2026-05-26

## 1. 核心原则

OAR 的默认安全原则：

> 先读后写、写前预演、执行前人工确认。

智能体可以自动观察、诊断、起草，但不能自动代表人做组织承诺。

人的角色见：[`human-role.md`](human-role.md)。

## 2. 执行安全模型

执行链路：

1. 智能体产生意图，例如“提醒 owner 更新 KR”。
2. OAR 将意图转成受控工具请求。
3. `LarkAdapter` 生成 dry-run 结果。
4. OAR 将 dry-run 结果展示为 `ProposedAction`。
5. 用户确认或编辑后确认。
6. 后端执行 allowlist 中的 CLI/OpenAPI 操作。
7. 写入 `AuditEvent`，并显示到审计时间线。

必须限制：

- CLI command allowlist。
- target object allowlist。
- scope allowlist。
- tenant/user context 校验。
- timeout、retry、rate limit。
- 输出长度限制和敏感信息脱敏。

禁止：

- LLM 直接执行任意原始命令。
- LLM 自动放宽 scope。
- 未经确认发送群消息、改 OKR、建会议、批量改任务。
- 外部 A2A 智能体直接拿到 CLI stdout/stderr 或任何 identity token。

## 3. 智能体能力边界

| 层级 | 智能体能做什么 | 是否自动执行 |
| --- | --- | --- |
| L1 观察 | 读取 OKR、任务、会议、文档、评论、更新时间 | 自动 |
| L2 诊断 | 判断风险、发现长期未更新的 KR、识别阻塞点、生成复盘 | 自动 |
| L3 建议 | 生成行动建议、更新草稿、评论草稿、任务草稿 | 自动生成，不写回 |
| L4 执行 | 写回 OKR、发评论、建任务、约会议、通知成员 | 必须用户确认 |
| L5 自主执行 | 预授权低风险动作 | MVP 不做 |

第一版可以做：

- 发现长期未更新的 OKR。
- 判断 KR 是否低于预期节奏。
- 根据任务、会议、文档生成每周 check-in 草稿。
- 提醒 owner 补充进展。
- 建议更新 KR 进度。
- 建议创建 owner 同步会议。
- 建议在飞书 OKR 下写进展评论。

第一版不应该做：

- 自动创建或删除 Objective。
- 自动修改 KR target、权重、owner、周期。
- 自动评价个人绩效。
- 自动群发敏感结论。
- 自动跨部门读取无权限数据。
- 自动执行批量 OKR 变更。

## 4. 权限与数据边界

基本原则：

- OAR 代表当前登录用户或被授权的企业应用读取飞书数据。
- 每个 request 必须绑定 `tenant_id`、`user_id`、scope 和 actor。
- manager 只能看到自己有权限查看的团队和成员 OKR。
- 外部 A2A 智能体不能直接读取原始 OKR 数据。
- 所有写回必须来自 `ConfirmedAction`。

数据分级：

| 数据 | 示例 | 处理规则 |
| --- | --- | --- |
| 原始 OKR | Objective、KR、owner、进度 | 仅 OAR 后端和授权用户可见 |
| 证据数据 | 文档摘要、会议纪要、任务状态 | 尽量摘要化，保留来源引用 |
| 智能体建议 | 风险、理由、建议动作 | 展示给授权用户，可进入待确认队列 |
| 审计数据 | actor、before/after、时间、结果 | append-only，禁止静默修改 |
| A2A 输出 | 每周简报、风险摘要、artifact | 默认脱敏和最小化 |

永不允许：

- 外部智能体直接持有飞书长期 token。
- 外部智能体绕过 OAR 写回飞书。
- 未经授权读取跨团队/跨部门 OKR。
- 在日志中输出 access token、refresh token、完整会议原文或敏感人事结论。

## 5. A2A 策略

引入 A2A 后，OAR 的长期形态是 **OKR 智能体中枢**。

协议分工：

- MCP / Lark CLI / OpenAPI 是工具层。
- A2A 是协作层。
- OAR 是权限、确认、审计和 OKR 领域策略的控制层。

OAR 对外可暴露的 A2A skills：

- `okr.risk_review`
- `okr.weekly_brief`
- `okr.progress_diagnosis`
- `okr.propose_action`
- `okr.audit_status`

外部 A2A 智能体：

- 只能通过授权 skill 读取摘要。
- 可以请求 OAR 生成建议。
- 永远不能直接写回飞书。
- 不能读取原始证据。
- 不能访问 OAR 记忆。
- 不能绕过待确认动作。

路线：

- 阶段 1-2：不开放外部 A2A，只做内部智能体工作流。
- 阶段 3：只读 A2A Server，输出每周简报、风险摘要、审计状态。
- 阶段 4：允许外部智能体提交 `ProposedAction`，但仍由 OAR 用户确认后执行。
