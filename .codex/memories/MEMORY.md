# OAR Project Distillation: AGENTS.md & MEMORY.md

## 1. 核心定位与业务边界

*   **业务定义**：飞书/Lark 租户的 **OKR 周度运营驾驶舱（收件箱模式）**。解决的是“目标执行与过程运营”，而非“制定 OKR”。
*   **非目标 (Out of Scope)**：非通用 OKR SaaS、非飞书 OKR 替代品、非绩效/HR 评估工具、非全自动决策 Agent。
*   **系统权威边界**：
    *   **飞书 (Lark)**：OKR 基础数据、文档、任务、日历、IM 消息的**原始权威数据源**。
    *   **OAR 后端**：评审风险队列、待办动作、审计日志、证据索引与决策反馈的**控制源**。

### MVP 功能范围划定

| 准入功能 (In-Scope) | 严禁功能 (Forbidden) |
| :--- | :--- |
| 读取飞书 OKR 周期、Objective、KR 及 Progress | 自动创建/删除 Objective |
| 跨应用同步证据链（文档、任务、会议纪要、IM、日历） | 自动修改 KR 目标值、权重、负责人或周期 |
| 生成周度简报、风险队列及建议动作 | 自动批量回写 / 无人值守执行 |
| **人机协同**：确认（Confirm）、编辑并确认、拒绝 | 外部 A2A（Agent-to-Agent）直连回写 |
| 回写已确认的进度、评论、任务、日程草稿 | 自动删除 Progress（MVP 仅支持 Dry-Run 校验） |
| 记录 `AuditEvent`（操作者、范围、目标、变更前后、结果） | 在日志/存储中泄露 Token、会议转录明文或 HR 敏感结论 |

---

## 2. 安全与权限高压线

> **核心原则**：Read first, dry-run before write, human confirmation before execution.

*   **执行链条**：`ConfirmedAction -> OperationLedger -> LarkAdapter -> AuditEvent`（一切飞书写入必须源自用户显式确认的 `ConfirmedAction`）。
*   **沙箱机制**：严禁 LLM 执行原生 Shell、任意 CLI 命令或未经安全审查的 OpenAPI。
*   **授权范围**：采用以用户为主体的默认执行身份，支持离线访问（`offline_access`）与用户 ID 读取（`auth:user.id:read`）。

---

## 3. 技术栈与架构设计

```
┌────────────────────────────────────────────────────────┐
│ UI 客户端 (SwiftUI + AppKit bridge)                     │
│ macOS 驾驶舱收件箱 (主界面) / iOS 审批轻量伴侣 (辅助)          │
└──────────────────────────┬─────────────────────────────┘
                           ▼
┌────────────────────────────────────────────────────────┐
│ OAR 核心后端 (Rust 7x24 调度常驻服务)                     │
│ 负责多源同步、风险检测、幂等账本 (OperationLedger)          │
└──────────────────────────┬─────────────────────────────┘
                           ▼
┌────────────────────────────────────────────────────────┐
│ 适配层 (LarkAdapter)                                    │
│ 生产环境：Rust 原生飞书 OpenAPI 适配器                     │
│ 测试验证：Lark CLI (v1.0.39) 仅用于 Fixture 录制与回归       │
└──────────────────────────┬─────────────────────────────┘
                           ▼
┌────────────────────────────────────────────────────────┐
│ 存储层 (Postgres + pgvector)                            │
│ 拒绝图数据库；采用三层关系表存储 ONTOLOGY, VECTOR, DECISION│
└────────────────────────────────────────────────┘
```

---

## 4. Phase 0.6 研发优先级 (当前聚焦)

*   **领域模型构建**：实现 Rust 核心领域模型（`Tenant`, `OarUser`, `LarkIdentity`, `TokenGrant`, `DeviceSession`, `OperationLedger`, `AuditEvent`）。
*   **安全认证落地**：实现加密 `TokenGrant` 存储、Refresh Token 原子性滚动更新、租户解绑与重新授权逻辑。
*   **防重防并发**：实现 `OperationLedger` 幂等执行，编写并发测试，确保同一 `ConfirmedAction` 有且仅能执行一次。
*   **适配器 Mock 编写**：将 Phase 0.5 沉淀 of Lark CLI 真实输出固化为 `LarkAdapter` 的本地 Fixture，实现解析器。

---

## 5. 已知的 Lark CLI (v1.0.39) 行为偏离与 Quirks

在解析本地 Fixture 或编写适配器时必须处理以下边界情况：
*   **`auth status`**：返回 JSON 文本，但传入 `--format json` 会直接报错。
*   **`auth check`**：必须显式传入 `--scope`。
*   **模拟写入 (Dry-Run)**：输出中可能包含非结构化的 `=== Dry Run ===` 纯文本前缀。
*   **`cycle-detail`**：返回的 Objective/KR `content` 字段是双重转义的 JSON 字符串，需要二次 Parse。
*   **`progress-list`**：返回的数据键名为 `data.progress_list[]` 而非直觉上的 `data.progress[]`。
*   **状态字段类型不一致**：真实 API 创建/更新响应中状态值为字符串（如 `"normal"`），而 Dry-Run Payload 里的状态值可能是数字。
*   **接口限制**：Lark CLI 中不支持原生的 `okr.progress_records.*` 结构体，必须通过系统快捷键和模拟校验规避。

---

## 6. OAR 三层记忆模型 (Memory Architecture)

1.  **Ontology Graph (关系型图谱)**：存储租户、用户、团队、OKR 周期、Objective、KR、任务、文档、实体关系（Postgres 物理表）。
2.  **Vector Memory (向量语义记忆)**：对文档摘要、会议纪要、任务流、周报等进行语义检索（pgvector 插件）。
3.  **Decision Memory (决策特征记忆)**：记录用户的历史确认、修改、拒绝行为，修正偏好并标定系统信任度。
*   **铁律**：**Memory 绝不等于直接证据。** 记忆仅用于检索召回和排序，所有的最终回写动作必须基于**当前的飞书实时数据证据**以及**用户的最终物理确认**。

---

## 7. 商业与可用性度量 (KPI)

*   **成功信号**：周会准备时间缩减 **50%**，建议采纳率 **>= 30%**，用户能明确溯源并信任证据链，**100%** 的回写有不可篡改的操作审计。
*   **警示信号**：用户周留存差（无法形成周度习惯）、建议确认率 < 10%、安全权限合规受阻、或者用户依然偏好纯粹的飞书原生界面。
