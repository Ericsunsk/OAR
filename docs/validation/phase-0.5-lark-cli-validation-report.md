# 阶段 0.5 Lark CLI 验证报告

更新日期：2026-05-26
执行时间：2026-05-26 10:07:40 +0800 - 10:26:12 +0800

## 1. 结论

阶段 0.5 已完成 `T0-T9` 的目标验证。

当前判断：

> `lark-okr` 可以作为 OAR 的 OKR 只读主路径，以及 progress 创建 / 更新写回路径。删除能力只允许 dry-run 预览，不进入 MVP 默认能力。

已验证能力：

- CLI 安装与登录态可用，版本为 `1.0.39`。
- user 和 bot 身份可用，当前默认身份为 user。
- OKR 读写相关 scope 已授权，`offline_access` 已授权。
- `+cycle-list` 可读取 OKR 周期。
- `+cycle-detail` 可读取 Objective / KR 主数据。
- `+progress-list` 可读取 progress 列表。
- `+progress-create --dry-run` 可预览创建请求。
- `+progress-create` 可真实创建测试 progress，并可读回。
- `+progress-update --dry-run` 可预览更新请求。
- `+progress-update` 可真实更新测试 progress，并可读回。
- `+progress-delete --dry-run` 可预览删除请求，未真实删除。

## 2. 测试结果

| 测试 | 目标 | 结果 | 说明 |
| --- | --- | --- | --- |
| T0 | CLI 与登录态 | 部分通过 | `auth status` 可返回当前 user 身份；当前 CLI 不支持 `auth status --format json` |
| T1 | Scope 检查 | 通过 | `okr:okr.period:readonly`、`okr:okr.content:readonly`、`okr:okr.content:writeonly` 均已授权 |
| T2 | Schema 快照 | 部分通过 | OKR shortcut 和部分原生 schema 可用；`okr.progress_records.*` 原生 schema 不存在 |
| T3 | 读取 OKR 周期 | 通过 | `+cycle-list` 返回测试周期 |
| T4 | 读取周期详情 | 通过 | `+cycle-detail` 返回 Objective 和 Key Result |
| T5 | 读取进展记录 | 通过 | `+progress-list` 可返回空列表或已有记录 |
| T6 | 创建进展 dry-run | 通过 | 返回 OpenAPI 路径和 payload，未产生真实写回 |
| T7 | 执行创建进展 | 通过 | 创建真实测试 progress，并可读回 |
| T8 | 更新进展 dry-run / 执行 | 通过 | 更新测试 progress，并可读回 |
| T9 | 删除进展 dry-run | 通过 | 返回 DELETE 请求预览，未真实删除 |

## 3. CLI 行为差异

阶段 0.5 发现了几个实现时必须处理的差异：

- `auth status` 输出本身是 JSON，但当前 CLI 不支持 `--format json`。
- `auth check` 需要使用 `--scope` 参数，例如 `lark-cli auth check --scope "okr:okr.content:readonly"`。
- `okr.progress_records.*` 原生 schema 在当前 CLI 中不可用，progress 能力依赖 shortcut 和 dry-run 验证。
- dry-run 输出带 `=== Dry Run ===` 前缀，`LarkAdapter` 需要兼容此前缀或寻找纯 JSON 输出方式。
- `cycle-detail` 返回的 Objective / KR `content` 是 JSON 字符串，需要二次解析为 ContentBlock。
- `progress-list` 返回字段是 `data.progress_list[]`，不是 `data.progress[]`。
- create / update 响应中 `progress_rate.status` 是字符串，例如 `normal`；dry-run payload 中 status 是数字，例如 `0`。

## 4. LarkAdapter 影响

`LarkAdapter` 最小 OKR 接口可以进入实现：

```text
list_okr_cycles(user_id, user_id_type) -> OkrCycle[]
get_okr_cycle_detail cycle_id -> OkrCycleDetail
list_progress(target_id, target_type) -> ProgressRecord[]
dry_run_create_progress(request) -> ToolDryRun
create_progress(confirmed_action_id, request) -> ProgressRecord
dry_run_update_progress(request) -> ToolDryRun
update_progress(confirmed_action_id, request) -> ProgressRecord
dry_run_delete_progress(request) -> ToolDryRun
```

实现要求：

- 所有写方法必须接收 `confirmed_action_id`。
- 所有写方法必须先有 dry-run 预览。
- `delete_progress` 在 MVP 中只开放 dry-run，不开放真实执行。
- CLI 输出必须经过 schema fixture 回归测试。
- 解析器必须兼容 dry-run 前缀和 ContentBlock 字符串。
- 不在日志中输出 token、完整 scope 或敏感原文。

## 5. 安全边界

阶段 0.5 中真实写回只发生在一次性测试 KR 上。

MVP 默认允许：

- 读取 OKR 周期、Objective、KR 和 progress。
- 对用户确认后的测试或低风险 progress 进行创建 / 更新。
- 对删除请求生成 dry-run 预览。

MVP 默认不允许：

- 自动创建或删除 Objective。
- 自动修改 KR target、权重、owner、周期。
- 自动删除 progress。
- 未经 `ConfirmedAction` 写回飞书。

## 6. 结论与下一步

阶段 0.5 已完成，`lark-okr` 主路径通过。

下一步进入阶段 0.6：

- 验证 `offline_access` 和 refresh 前置条件。
- 设计 `TokenGrant` 加密存储与 refresh token rotation。
- 设计 `OperationLedger`，保证同一 `ConfirmedAction` 只执行一次。
- 将本次 CLI 输出固化为 `LarkAdapter` fixture。
