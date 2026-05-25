# OAR 产品计划

更新日期：2026-05-26
当前状态：方向判断 + MVP 定义 + 阶段 0.5 验证准备
配套执行清单：[`docs/lark-cli-capability-matrix.md`](lark-cli-capability-matrix.md)

## 0. 核心结论

OAR 值得继续做，但它不应该是通用 OKR SaaS，也不应该是通用智能体桌面。

最清晰的产品切口是：

> 面向飞书企业租户的 **OKR 复盘驾驶舱**：每周自动发现 OKR 执行风险，汇总证据，生成行动建议，并在用户确认后安全写回飞书。

长期形态是：

> 飞书 OKR 的 **AI 幕僚长 + 智能体网关**：负责盯进展、找风险、起草动作，并让其他智能体在 A2A 边界内协作；关键写回永远由人确认。

一句话原则：

> 不是让用户“管理 OKR”，而是让用户每周 10 分钟清空 OKR 执行风险队列。

当前最重要的下一步：

> 完成阶段 0.5，用真实飞书租户验证 `lark-okr` 的读/写、scope、dry-run、schema 能力。未验证前，所有 Lark CLI 能力都只能视为设计假设。

## 1. 产品决策

| 主题 | 决策 | 原因 |
| --- | --- | --- |
| 产品定位 | OKR 复盘驾驶舱 | 避开通用 OKR SaaS 和通用智能体桌面，聚焦飞书内的 OKR 执行运营 |
| 权威数据源 | 飞书 OKR | OAR 不替代飞书，只做智能体工作流层 |
| MVP 场景 | 每周 OKR 复盘 | 高频、可验证、能体现智能体价值 |
| 默认入口 | 复盘收件箱 | 避免变成低频仪表盘 |
| 写回策略 | 先读后写、写前预演、执行前人工确认 | 建立企业信任和审计边界 |
| 技术主线 | Swift 原生前端 + Rust 后端/core + LarkAdapter | 保持原生体验、强类型工具层和可审计执行 |
| Lark 集成 | LarkAdapter 优先；OKR 路径优先验证 Lark CLI，保留 OpenAPI 兜底路径 | Lark CLI 适合智能体工具层，但生产必须可替换 |
| 7x24 运行 | 后端智能体运行时，而不是桌面端常驻 | 客户端负责交互和审批，后台负责调度、同步、审计 |
| A2A | 阶段 3 以后只读开放 | MVP 先闭环飞书 OKR，避免过早扩大安全面 |

## 2. 已验证与待验证假设

| 假设 | 状态 | 当前判断 |
| --- | --- | --- |
| 飞书 OKR 仍是主要权威数据源 | 公开资料支持 | 成立，OAR 应叠加在飞书之上 |
| Lark CLI 可作为智能体工具层 | 公开资料支持 | 方向成立，但必须锁定版本、做 schema 快照和回归验证 |
| `lark-okr` 覆盖 OKR 周期、Objective、KR、进展记录 | 部分公开资料支持 | 必须用真实租户冒烟测试验证 |
| Lark OAuth 可绑定用户身份 | 公开文档支持 | 可以做用户代理身份 |
| 后台 7x24 可代表用户工作 | 有条件成立 | 依赖 `offline_access`、refresh token rotation 和企业合规授权 |
| 飞书事件订阅可支撑响应式智能体 | 未充分验证 | MVP 采用定时任务优先，事件订阅作为增强 |
| 用户愿意每周打开复盘收件箱 | 未验证 | 需要 3-5 个真实经理 / PMO 做 2-4 周陪跑式 MVP |
| 企业允许保存 token、证据摘要、记忆 | 未验证 | 先做单租户 / 内部试点，并设计删除、导出、保留策略 |
| 记忆能显著提升建议质量 | 未验证 | 需要历史 OKR 复盘回归用例 |

## 3. 市场判断

已看到的相似方向：

| 产品/类型 | 相似点 | OAR 应避开的正面战场 |
| --- | --- | --- |
| WorkBoardAI | AI 战略执行、会前材料、计分卡 | 企业战略执行大平台 |
| Tability / Tabby AI | OKR 智能体、check-in、行动建议、MCP | 通用 OKR 智能体 |
| Rhythms.ai | AI 原生 OKR、提醒、自动进度 | AI OKR 平台 |
| Betterworks / Leapsome / Lattice | OKR + 绩效管理 | HR / 绩效管理 |
| 飞书 OKR | OKR 权威数据源 | OKR CRUD 和组织权限 |
| OpenClaw | 桌面智能体驾驶舱、skills、工具管理 | 通用智能体工作区 |

结论：

- AI 生成 OKR、自动报告、check-in 提醒已不稀缺。
- OAR 的差异点必须是飞书租户内的深集成、权限继承、证据链、确认写回和审计。
- 不碰绩效评价是必要边界，否则会进入高敏 HR 场景，降低试点速度。
- 桌面端可以借鉴 OpenClaw 的驾驶舱形态，但 OAR 必须保持 OKR 垂直，不做通用电脑控制。

## 4. 目标用户

| 用户 | 核心场景 | 痛点 | OAR 价值 |
| --- | --- | --- | --- |
| 创始人 / CEO | 每周看公司目标风险 | 信息散在飞书各处，风险暴露晚 | 看到本周有风险的 OKR 和建议动作 |
| 经理 / 团队负责人 | 跟进团队 OKR | owner 不更新，KR 进展缺证据 | 自动生成复盘和跟进行动 |
| PMO / 幕僚长 | 运营组织 OKR 节奏 | 周报、催更、复盘靠人工 | 把 OKR 运营变成收件箱和审计流 |
| AI 智能体 / 外部系统 | 获取 OKR 上下文或提交建议 | 不知道如何安全读写 OKR | 通过 A2A 请求受控 OKR skills |

优先 ICP：

> 20-300 人、重度使用飞书、已有 OKR 节奏但执行运营靠人工的团队。第一批最好是内部团队或设计伙伴，不建议直接做开放 SaaS。

## 5. MVP 定义

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

## 6. 桌面端形态

桌面端应该是 **OKR 复盘驾驶舱**，不是报表首页，也不是聊天首页。

推荐三栏布局：

| 区域 | 作用 |
| --- | --- |
| 左侧侧边栏 | 团队、周期、风险视图、待确认动作、审计、智能体网络 |
| 中间主区 | OKR 复盘看板、风险排序、目标详情、KR 进度、证据链 |
| 右侧智能体面板 | 智能体解释、建议、建议动作、确认/拒绝、执行日志 |

默认入口：

- 复盘收件箱。
- 本周 OKR 风险榜。
- 待确认智能体动作。
- 每周简报。

点进单个 OKR 后，用户应该看到：

- Objective / KR 当前状态。
- 智能体判断：为什么有风险。
- 证据：更新时间、任务进度、会议纪要、文档变更、owner check-in。
- 智能体建议：更新进度、写评论、提醒 owner、创建跟进任务。
- 操作：`确认`、`编辑后确认`、`拒绝`、`询问智能体`。

桌面端不是“万能聊天框”。聊天只能作为解释、调整、追问的辅助能力，主工作流必须是收件箱、队列、时间线和确认。

## 7. iOS 与飞书入口

iOS 只做轻量伴随端：

- 查看今日风险。
- 接收提醒。
- approve/reject 待确认动作。
- 快速查看每周简报。

飞书入口是高优先级：

- Bot 做触达。
- 消息卡片做确认。
- Shortcut 做快速复盘。
- 重要动作回到 OAR 或飞书卡片确认。

产品心智：

> OAR 智能体在后台持续准备工作；用户只在需要判断和授权时出现。

## 8. 原生技术形态

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

## 9. Lark CLI 优先原则

OAR 是基于 Lark CLI 开发的应用，但不能只是“调用 CLI”。业务层必须 **LarkAdapter 优先**，OKR 主路径在阶段 0.5 优先验证 Lark CLI，失败时切换或降级到 OpenAPI。

核心技术原则是：

> 把 Lark CLI 产品化成安全、可审计、可回滚的智能体工具层。

公开资料支持的方向：

- Lark CLI 覆盖 Messenger、Docs、Drive、Sheets、Base、Calendar、Meetings、Minutes、Mail、Tasks、Wiki、Contacts、Approval、Attendance、OKR 等核心域。
- 官方资料显示其支持大量 commands、智能体 skills、dry-run、schema 查看、授权检查、身份切换。
- `lark-okr` 是 OKR 主路径的优先验证对象。

保守原则：

- 当前能力以本机安装版本的 schema 快照为准。
- 生产环境锁定 CLI 版本，不自动升级。
- 所有调用经过 `LarkAdapter`。
- 每个 command 都要有 allowlist、超时、输出长度限制、脱敏和审计。
- OpenAPI 兜底路径必须设计进架构，不能把 CLI 当成不可替换黑盒。

阶段 0.5 详细验证见：[`docs/lark-cli-capability-matrix.md`](lark-cli-capability-matrix.md)

## 10. LarkAdapter

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

## 11. 账号、身份与 7x24

OAR 不应该依赖桌面端常驻来实现智能。更合理的形态是：

> macOS / iOS / 飞书入口负责交互、查看和审批；后端智能体运行时负责 7x24 调度、同步、审计和工具执行。

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

## 12. 多端同步

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

## 13. 智能体运行时、模型配置与记忆

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

记忆分层：

| 记忆类型 | 内容 | 用途 |
| --- | --- | --- |
| 用户偏好记忆 | 简报风格、提醒偏好、常用确认方式 | 个性化输出 |
| 团队运作记忆 | 团队节奏、owner 分工、例会规律 | 理解团队上下文 |
| OKR 周期记忆 | 本周期目标、历史复盘、progress 变化 | 跨周风险判断 |
| 决策记忆 | 用户确认/拒绝过的动作和原因 | 避免重复建议 |
| 策略记忆 | 企业禁止动作、审批规则、scope 约束 | 防止越权 |

记忆必须支持关闭、删除、导出、重建、保留策略和租户隔离。MVP 可以先只做“决策记忆”和“OKR 周期记忆”，避免过早做复杂个人画像。

## 14. 证据链

OAR 的每条智能体判断都必须绑定来源。

| 智能体判断 | 可用证据来源 |
| --- | --- |
| KR 长期未更新 | OKR 更新时间、owner 最近消息、任务更新时间 |
| KR 低于预期节奏 | OKR progress、任务完成率、会议/文档中的阻塞点 |
| owner 未响应 | IM 搜索、任务评论、OKR 评论、会议纪要 |
| 目标缺证据 | Docs/Wiki/Minutes/Tasks 中找不到关联上下文 |
| 需要跟进 | Minutes 待办、Calendar 空档、Tasks 未完成项 |
| 可以更新 progress | 已完成任务、会议结论、owner check-in、Base/Sheets 指标 |

证据存储最小字段：

- `evidence_id`
- `tenant_id`
- `source_type`
- `source_ref`
- `source_url`
- `source_timestamp`
- `collected_at`
- `linked_okr_id`
- `linked_kr_id`
- `owner_user_id`
- `extracted_summary`
- `raw_excerpt_hash`
- `confidence`
- `visibility_scope`
- `retention_policy`

MVP 原则：优先保存摘要、来源引用和 hash，不默认保存完整原文。

## 15. 执行安全模型

默认原则：

> 先读后写、写前预演、执行前人工确认。

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

## 16. 智能体能力边界

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

## 17. A2A 策略

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

## 18. 权限与数据边界

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

## 19. 路线图

| 阶段 | 目标 | 交付物 | 通过 / 不通过 |
| --- | --- | --- | --- |
| 阶段 0：验证原型 | 证明交互闭环可行 | SwiftUI shell、Rust 后端骨架、OKR 列表、智能体复盘、确认、审计 | 用户理解驾驶舱形态 |
| 阶段 0.5：Lark CLI 能力验证 | 决定 `LarkAdapter` 主路径 | `docs/lark-cli-capability-matrix.md`、`lark-okr` 冒烟测试、schema、权限错误映射 | `T0-T6` 通过，写回至少 dry-run 可用 |
| 阶段 0.6：身份与同步验证 | 验证 Lark 登录、代理身份、多端同步和后台任务 | OAuth grant、TokenGrant、DeviceSession、OperationLedger、定时 worker | `offline_access` 与状态同步成立 |
| 阶段 0.7：智能体运行时验证 | 验证模型、记忆、证据存储能否稳定生成建议 | 模型配置、prompt/policy 版本、证据 schema、记忆 schema、历史案例回放 | 建议质量可回放评估 |
| 阶段 1：内部 OAR 智能体 | 跑通真实每周复盘 | OKR 读取、每周简报、待确认队列、多端确认、轻量写回 | 2-4 周真实使用 |
| 阶段 2：飞书工作流闭环 | 让用户在飞书内处理动作 | Bot、消息卡片、审计时间线、证据链 | 卡片确认不低于桌面端 |
| 阶段 3：只读 A2A Server | 对外提供 OKR 摘要能力 | Agent Card、`weekly_brief`、`risk_review`、`audit_status` | 外部智能体只能读摘要 |
| 阶段 4：A2A 建议 / 客户端 | 与其他智能体双向协作 | 外部智能体创建 `ProposedAction`，OAR 调用其他领域智能体 | 仍不允许外部智能体直接写飞书 |

当前阶段 0.5 状态：

- 已创建 `docs/lark-cli-capability-matrix.md`。
- 本机尚未安装 `lark-cli`。
- 尚未接入真实飞书租户和测试 OKR。
- 写回测试必须等待一次性测试 Objective / KR 准备完成后执行。

接下来 7 天的推荐动作：

1. 准备真实飞书测试租户、测试用户、一次性测试 Objective / KR。
2. 安装并登录 `lark-cli`，记录 CLI version、identity、granted scopes。
3. 按 `docs/lark-cli-capability-matrix.md` 执行 `T0-T6`，先完成只读和 dry-run。
4. 根据结果决定 OKR 主路径：`lark-okr` 主路径、OpenAPI 主路径或只读 MVP。
5. 基于 `docs/product-plan.md` 编写 architecture/security 文档，统一 LarkAdapter、身份、同步和审计口径。
6. 搭建模拟每周复盘原型，验证 macOS 驾驶舱的信息架构是否顺手。

## 20. MVP 验证实验

第一阶段不要验证完整技术栈，而要验证用户是否真的需要每周 OKR 复盘驾驶舱。

陪跑式 MVP：

1. 找 3-5 位真实目标用户：经理、PMO、幕僚长、创始人。
2. 选 1-2 个真实团队和一个 OKR 周期。
3. 手动或半自动读取飞书 OKR、任务、会议、文档更新。
4. 用 OAR 原型生成每周简报、风险榜和建议动作。
5. 在飞书或 macOS 原型中让用户确认、编辑或拒绝动作。
6. 记录用户是否愿意下周继续使用，以及哪些建议被采纳。

通过信号：

- 复盘准备时间减少 50%。
- 建议动作的确认或编辑后确认比例达到 30%+。
- 用户能解释为什么相信或不相信证据链。
- 每周至少 1 次主动复盘会话。
- 用户愿意让 OAR 继续进入下一个 OKR 周期。

## 21. 成功指标

| 目标 | 指标 |
| --- | --- |
| 节省复盘时间 | 每周 OKR 复盘准备时间减少 50% |
| 提高风险发现 | 有风险 KR 在周会前被识别的比例提升 |
| 建立信任 | 建议动作确认率达到 30%-50% |
| 形成习惯 | 每周至少 1 次主动复盘会话 |
| 安全可控 | 100% 写操作有确认记录和审计事件 |
| 飞书闭环 | 已确认动作成功写回飞书比例稳定 > 95% |
| 多端一致性 | 同一个待确认动作在 macOS / iOS / 飞书卡片中状态一致 |
| 常驻可靠性 | 定时每周复盘任务按时完成率 > 95% |
| 身份可信 | 100% 写操作能追溯到授权用户、执行身份、scope 和已确认动作 |
| 记忆有效性 | 被用户拒绝过的重复建议明显减少 |

## 22. 风险与缓解

| 风险 | 影响 | 缓解 |
| --- | --- | --- |
| 飞书 OKR API 写权限难申请 | 无法完整写回 | MVP 先支持只读洞察 + 评论/任务/提醒写回 |
| Lark CLI 能力与权限不稳定 | 证据链无法闭环 | 阶段 0.5 单独验证 `lark-okr`、Docs、Tasks、Minutes、Calendar、IM |
| 未获得 `offline_access` | 无法后台 7x24 刷新用户凭证 | 阶段 0.6 通过/不通过；缺失时降级为用户在线触发 |
| OAuth 实际 scope 被裁剪 | 智能体以为有权限但执行失败 | 以 token 响应 `scope` 为准，建立缺失 scope 引导和兜底路径 |
| 用户不信任 AI 判断 | 确认率低 | 每条建议必须给证据和 before/after |
| 变成又一个仪表盘 | 使用频率低 | 首页必须是复盘收件箱，不是指标大屏 |
| 组织数据敏感 | 企业不敢授权 | scope 最小化、tenant 隔离、审计、人工确认 |
| refresh token 泄露或滥用 | 极高安全风险 | 加密存储、最小 scope、rotation、revoke、日志脱敏、异常使用告警 |
| 多端重复确认 | 重复发消息或重复写回 | `OperationLedger`、`idempotency_key`、后端状态机 |
| 记忆泄露或跨租户污染 | 高敏数据风险 | tenant 隔离、保留策略、可删除/导出、A2A 默认不读记忆 |
| A2A 过早复杂化 | 产品范围失控 | 阶段 3 前只做内部智能体，不开放外部 A2A |
| 与飞书原生 OKR 重叠 | 价值不清 | 明确不做 OKR CRUD，只做执行风险和行动闭环 |

## 23. 停止或转向标准

如果出现以下情况，应考虑停止或转向：

- 拿不到足够的飞书 OKR / 任务 / 文档读取权限，导致无法形成证据链。
- `lark-okr` 与 OKR OpenAPI 都无法稳定拿到 OKR 主数据。
- 无法取得 `offline_access` 或无法合规保存/刷新用户授权，导致 7x24 后端智能体不能成立。
- 用户每周不愿打开复盘收件箱，说明工作流切口不成立。
- 建议动作的确认率长期低于 10%，说明建议不可用或不可信。
- 多端同步无法避免重复执行或状态不一致。
- 记忆和证据存储无法让建议质量随周期提升。
- 写回权限长期无法落地，且只读简报不能形成稳定使用习惯。
- 企业更愿意直接在飞书内完成全部流程，独立桌面端没有明显效率增益。

## 24. 未来判断

**有未来，但它不是一个大众软件机会，而是一个垂直企业工作流机会。**

它的未来取决于三个判断是否成立：

1. OKR 的真正痛点不是创建，而是执行运营。
2. 飞书生态足够封闭且足够丰富。
3. 智能体的价值在“起草 + 证据 + 确认”，不是全自动。

明确判断：

> 值得继续做，但第一版必须极窄：只做每周 OKR 复盘驾驶舱。先证明“每周 10 分钟清空 OKR 风险队列”这件事有人愿意持续使用，再谈 A2A 和更大的智能体中枢。

如果每周循环跑通，OAR 有机会从一个飞书内部工具长成 OKR 智能体中枢；如果跑不通，继续加智能体、A2A、原生端都只是把复杂度堆高。

## 25. 参考来源

- WorkBoardAI OKR Creation & Alignment：https://www.workboard.com/products/cos-okr-creation-alignment
- WorkBoard AI Strategy Execution & OKR Platform：https://www.workboard.com
- WorkBoard AI agents for strategy execution：https://www.workboard.com/what-are-agents
- Tability Tabby AI OKR Agent：https://www.tability.io/features/ai/okr-agent
- Tability 介绍 OKR Agent：https://www.tability.io/odt/articles/introducing-the-very-first-ai-agent-dedicated-to-okrs
- Tability Agentic OKRs：https://www.tability.io/odt/articles/agentic-okrs
- Rhythms.ai OKR Platform：https://www.rhythms.ai/platform/goals
- Rhythms.ai AI-native OKR Platform：https://www.rhythms.ai/solutions/okr-platform
- Betterworks OKR Software：https://www.betterworks.com/product/okr-software
- Leapsome Goals and OKRs：https://www.leapsome.com/product/goals/okrs
- Lattice OKR Software：https://lattice.com/platform/goals/okrs
- 飞书 OKR：https://okr.feishu.cn
- Feishu OKR User Guide：https://www.feishu.cn/hc/en-US/articles/360049067622-okr-user-guide
- Lark OKR User Guide：https://www.larksuite.com/hc/en-US/articles/854393465133-okr-user-guide
- 飞书获取 user_access_token：https://open.feishu.cn/document/authentication-management/access-token/get-user-access-token?lang=zh-CN
- 飞书刷新 user_access_token：https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/authentication-management/access-token/refresh-user-access-token
- Lark access credentials：https://open.larksuite.com/document/home/introduction-to-scope-and-authorization/access-credentials
- 飞书事件订阅概览：https://open.feishu.cn/document/server-docs/event-subscription-guide/overview
- Lark WebSocket event subscription：https://open.larksuite.com/document/ukTMukTMukTM/uYDNxYjL2QTM24iN0EjN/event-subscription-configure-/use-websocket
- 飞书 ISV 管理规范：https://open.feishu.cn/document/uAjLw4CM/uMzNwEjLzcDMx4yM3ATM/isv-management-specifications
- Microsoft Viva Goals retirement：https://learn.microsoft.com/en-us/viva/goals/goals-retirement
- Microsoft Viva Goals retirement FAQ：https://learn.microsoft.com/en-us/viva/goals/goals-retirement-faq
- Lark CLI 官方文档：https://open.larksuite.com/document/mcp_open_tools/feishu-cli-let-ai-actually-do-your-work-in-feishu
- Lark CLI GitHub：https://github.com/larksuite/cli
- Lark CLI `lark-okr` skill：https://github.com/larksuite/cli/blob/main/skills/lark-okr/SKILL.md
- Agent2Agent (A2A) GitHub：https://github.com/a2aproject/A2A
- A2A v1.0.0 specification：https://a2a-protocol.org/v1.0.0/specification
- A2A specification：https://github.com/a2aproject/A2A/blob/main/docs/specification.md
- Google A2A Codelab：https://codelabs.developers.google.com/intro-a2a-purchasing-concierge
- Linux Foundation A2A announcement：https://www.linuxfoundation.org/press/linux-foundation-launches-the-agent2agent-protocol-project-to-enable-secure-intelligent-communication-between-ai-agents
- Microsoft Agent Framework：https://learn.microsoft.com/en-us/agent-framework
- OpenClaw GitHub：https://github.com/openclaw/openclaw
