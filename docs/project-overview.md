# OAR 项目概览

更新日期：2026-05-26  
当前状态：Phase 0.5 已完成；Phase 0.6（identity/auth refresh/multi-device sync/idempotent execution/auditability）进行中

## 1. 一句话定位

OAR 是面向飞书企业租户的 **OKR 复盘驾驶舱**：每周自动发现 OKR 执行风险，汇总证据，生成行动建议，并在用户确认后安全写回飞书。

它不是通用 OKR SaaS，不替代飞书 OKR，也不是绩效评价系统。

## 2. 核心决策（source-of-truth）

| 主题 | 决策 | 说明 |
| --- | --- | --- |
| 产品定位 | OKR 复盘驾驶舱 | 聚焦飞书内执行运营，不做通用 OKR 平台 |
| 权威数据源 | 飞书 | Lark 是原始数据权威；OAR 负责复盘、待办动作、审计与决策 |
| MVP 场景 | 每周复盘收件箱 | 默认入口是队列，不是聊天框/仪表盘 |
| 写回策略 | 先读后写、写前 dry-run、执行前人工确认 | 所有写回必须来自 `ConfirmedAction` |
| 技术主线 | Swift 客户端 + Rust core/backend + `LarkAdapter` | 生产飞书集成走 Rust 原生 OpenAPI adapter |
| Lark CLI 角色 | 仅验证/fixture/回归 | 不作为生产调用路径 |
| 运行方式 | 后台服务持续运行，客户端负责交互审批 | 不依赖桌面端常驻 |
| A2A 路线 | 先闭环复盘，再开放只读，再受控提交建议 | 外部写回仍由 OAR 人工门禁收口 |

长期形态是飞书 OKR 的 **AI 幕僚长 + 智能体网关**：负责盯进展、找风险、起草动作，并让其他智能体在 A2A 边界内协作；关键写回永远由人确认。

## 3. 市场与用户

目标 ICP：20-300 人、重度使用飞书、已有 OKR 节奏但执行运营靠人工的团队。

第一批角色：
- 创始人 / CEO：周会前快速看到目标风险。
- 经理 / 团队负责人：识别卡住 KR 与跟进动作。
- PMO / 幕僚长：把催更与复盘从人工表格变成可追溯队列。
- AI 智能体 / 外部系统（阶段 3+）：只能读取脱敏 OKR 上下文或提交建议，不能直接写回。

差异化边界：
- 做：权限继承、证据链、确认写回、审计可追溯。
- 不做：OKR CRUD 替代、绩效评价、自动批量写回、通用智能体桌面。

## 4. MVP 范围

必须包含：
- 读取 OKR 周期、Objective、KR、progress。
- 风险识别（长期未更新、低进度、缺更新 owner）。
- 聚合 Docs/Tasks/Meetings/Minutes/Calendar/IM 证据摘要。
- 生成周报、风险队列、建议动作。
- 用户确认后写回进度、评论、提醒、任务或会议草稿。
- 全链路 `AuditEvent`（actor/scope/target/before-after/result）。

明确不做：
- OKR 创建器、绩效评价、自动批量写回、外部 A2A 直接写回飞书。

## 5. 阶段状态与风险

Phase 0.5：已完成 Lark CLI/OKR 读取与 progress 创建/更新验证，删除保持 dry-run；生产主路径已收敛到 Rust 原生 OpenAPI adapter。  
Phase 0.6：token refresh service、Postgres Recorder、audit 写入、`run_once` 幂等链路、真实 Rust/Reqwest refresh adapter、后台 maintenance daemon、last-device logout 本地 grant revoke + append-only audit 已接入；已加入默认关闭的真实 Feishu refresh smoke 入口，仍需用一次性测试授权实际运行，并继续补齐故障恢复与运维闭环验证。

当前关键假设：

| 假设 | 状态 | 当前判断 |
| --- | --- | --- |
| 飞书 OKR 是权威数据源 | 公开资料与 Phase 0.5 支持 | OAR 应叠加在飞书之上 |
| Lark CLI 可作为验证和 fixture 工具 | 已验证 | 生产主路径仍转向 Rust 原生 OpenAPI adapter |
| Lark OAuth 可绑定用户身份 | 已初步验证 | `offline_access` 已授权；production `TokenGrant`、refresh rotation、last-device logout 本地 revoke/audit 已接入 |
| 用户愿意每周打开复盘收件箱 | 未验证 | 需要 3-5 个真实经理 / PMO 做 2-4 周陪跑 |
| 记忆能显著提升建议质量 | 未验证 | 需要历史 OKR 复盘回归用例 |

当前主要风险：
1. 真实 Feishu refresh/live smoke 已有 env-gated 入口，但仍需用测试授权定期执行，避免 fake fixture 与飞书实际响应漂移。
2. 幂等执行的失败恢复覆盖不足（重试、超时、revoke/reauth）。
3. 多端一致性与离线期间后台持续运行仍需真实流程验证。
4. “每周 10 分钟清空风险队列”的持续使用习惯仍待真实团队验证。

成功信号：
- 1 个真实团队连续 2-4 周使用。
- 复盘准备时间减少 50%。
- 建议动作确认或编辑后确认比例达到 30%+。
- 用户能解释为什么相信或不相信证据链。
- 100% 写操作有确认记录和审计事件。

回炉信号：
- 用户不愿每周打开复盘收件箱。
- 建议确认率长期低于 10%。
- 证据链无法让用户建立信任。
- 独立驾驶舱相对飞书内流程没有效率增益。

## 6. 近期路线图（7 天）

1. 用一次性测试授权运行真实 Feishu refresh live smoke；确认飞书是否提供官方 OAuth provider revoke endpoint，若无则保持本地 grant revoke 边界清晰。
2. 扩展 Postgres Recorder + `OperationLedger` + `run_once` 的并发和重试验证。
3. 收敛审计事件结构与落库策略，补足关键失败场景可追溯性。
4. 接入 scheduler/daemon 到真实任务流，验证客户端离线期间连续性。
5. 组织真实团队回归，验证 macOS/iOS/飞书入口的状态同步与周节奏使用。

## 7. 文档地图

- [复盘收件箱](review-inbox.md)
- [系统架构](system-architecture.md)
- [飞书集成验证（Phase 0.5）](feishu-integration.md)
- [身份与同步验证（Phase 0.6）](identity-auth-sync.md)
- [执行与审计边界](execution-audit.md)
- [记忆与证据链](memory-evidence.md)
- [验证计划](validation-plan.md)
- [参考资料](reference/references.md)
