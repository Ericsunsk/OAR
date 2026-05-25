# Lark CLI 能力验证矩阵

更新日期：2026-05-26

## 1. 目标

本文件用于验证 OAR 阶段 0.5 的核心假设：

> `lark-okr` 能否作为 OAR 读取飞书 OKR、构建证据链、生成 `ProposedAction` 并在用户确认后安全写回的主路径。

验证结论将决定：

- `LarkAdapter` 是否 CLI 优先。
- OKR 主数据是否优先走 `lark-okr`。
- 写回能力是否只允许进展/评论/任务/提醒。
- 是否必须把 OKR OpenAPI 作为主路径或兜底路径。

## 2. 当前状态

| 项目 | 状态 | 说明 |
| --- | --- | --- |
| 本机 `lark-cli` | 未安装 | `command -v lark-cli` 无输出。 |
| 真实飞书租户 | 未接入 | 需要一个可测试 OKR 的企业租户。 |
| OAuth 授权 | 未完成 | 需要用户完成 Lark / Feishu OAuth。 |
| 写回测试对象 | 未准备 | 必须使用一次性测试 OKR 或测试周期，不能对真实业务 OKR 直接写入。 |
| 本文件 | 已创建 | 可作为阶段 0.5 执行清单和结果记录表。 |

## 3. 验证原则

- 先读后写：先验证只读能力，再验证写能力。
- 写前预演：所有写操作先跑 `--dry-run`。
- 仅限一次性测试对象：写操作只允许在测试 Objective / KR / progress record 上执行。
- 执行前人工确认：真实写回前必须人工确认目标、内容、身份和 scope。
- 全量审计：每一步记录 command、identity、scope、target、result、error。
- 不记录 token：日志中禁止保存 access token、refresh token、authorization code。

## 4. 前置准备

### 4.1 安装与登录

```bash
npx @larksuite/cli@latest install
lark-cli config init --new
lark-cli auth login --recommend
lark-cli auth status
```

记录：

| 字段 | 值 |
| --- | --- |
| `lark_cli_version` | 待填写 |
| `install_source` | npm / source / other |
| `tenant_id` | 待填写 |
| `identity_type` | user / bot |
| `granted_scopes` | 待填写 |
| `auth_status_result` | pass / fail |

### 4.2 测试对象

需要准备：

- 1 个测试用户。
- 1 个当前 OKR 周期。
- 1 个测试 Objective。
- 1 个测试 Key Result。
- 1 条可创建、更新、删除的测试 progress record。

建议环境变量：

```bash
export OAR_TEST_USER_ID="ou_xxx"
export OAR_TEST_USER_ID_TYPE="open_id"
export OAR_TEST_CYCLE_ID="1234567890123456789"
export OAR_TEST_OBJECTIVE_ID="2345678901234567890"
export OAR_TEST_KEY_RESULT_ID="3456789012345678901"
export OAR_TEST_PROGRESS_ID="4567890123456789012"
```

## 5. 能力矩阵

| 能力 | CLI / API | 所需 scope | 身份 | 状态 | 决策 |
| --- | --- | --- | --- | --- | --- |
| 授权状态 | `lark-cli auth status` | auth scopes | user | 未验证 | P0 |
| Scope 检查 | `lark-cli auth check` | target scope | user | 未验证 | P0 |
| Schema 查看 | `lark-cli schema ...` | none / related scope | user / bot | 未验证 | P0 |
| OKR 周期列表 | `okr +cycle-list` | `okr:okr.period:readonly` | user | 未验证 | P0 |
| OKR 周期详情 | `okr +cycle-detail` | `okr:okr.content:readonly` | user | 未验证 | P0 |
| 进展记录列表 | `okr +progress-list` | `okr:okr.content:readonly` | user | 未验证 | P0 |
| 创建进展 dry-run | `okr +progress-create --dry-run` | `okr:okr.content:writeonly` | user | 未验证 | P0 |
| 创建进展执行 | `okr +progress-create` | `okr:okr.content:writeonly` | user | 未验证 | P0 |
| 更新进展 dry-run | `okr +progress-update --dry-run` | `okr:okr.content:writeonly` | user | 未验证 | P0 |
| 更新进展执行 | `okr +progress-update` | `okr:okr.content:writeonly` | user | 未验证 | P0 |
| 删除进展 dry-run | `okr +progress-delete --dry-run` | `okr:okr.content:writeonly` | user | 未验证 | P1 |
| 删除进展执行 | `okr +progress-delete` | `okr:okr.content:writeonly` | user | 禁止默认执行 | P2 |

## 6. 冒烟测试

### T0：CLI 与登录态

目的：确认本地 CLI、身份和授权状态可用。

```bash
lark-cli auth status --format json
```

通过标准：

- 返回当前登录用户。
- 能看到已授权 scopes。
- 不输出 token 明文。

失败处理：

- 如果未登录，重新执行 `lark-cli auth login --recommend`。
- 如果 scope 不足，记录缺失 scope，不要继续写测试。

### T1：Scope 检查

目的：确认 OKR 读/写 scope 是否可用。

```bash
lark-cli auth check "okr:okr.period:readonly"
lark-cli auth check "okr:okr.content:readonly"
lark-cli auth check "okr:okr.content:writeonly"
```

通过标准：

- 读取 scopes 必须通过。
- 写入 scope 如果失败，MVP 降级为只读洞察 + 任务/提醒/评论写回。

### T2：Schema 快照

目的：确认当前 CLI 版本的 OKR schema、支持身份和所需 scopes。

```bash
lark-cli okr --help
lark-cli schema okr.cycles.list
lark-cli schema okr.cycle.objectives.list
lark-cli schema okr.progress_records.create
```

记录：

| 字段 | 值 |
| --- | --- |
| `schema_snapshot_time` | 待填写 |
| `schema_hash` | 待填写 |
| `supported_identities` | 待填写 |
| `required_scopes` | 待填写 |

如果 schema method 名称与本文不同，以当前 `lark-cli okr --help` 和 `lark-cli schema` 输出为准。

### T3：读取 OKR 周期

目的：确认能为测试用户列出 OKR 周期。

```bash
lark-cli okr +cycle-list \
  --user-id "$OAR_TEST_USER_ID" \
  --user-id-type "$OAR_TEST_USER_ID_TYPE" \
  --format json
```

通过标准：

- 返回 `cycles` 数组。
- 至少包含一个可用 `cycle_id`。
- 返回内容包含周期起止时间和状态。

### T4：读取周期详情

目的：确认能读取 Objective / KR 主数据。

```bash
lark-cli okr +cycle-detail \
  --cycle-id "$OAR_TEST_CYCLE_ID" \
  --format json
```

通过标准：

- 返回 `objectives` 数组。
- 每个 Objective 至少包含 `id`、`owner`、`content`、`key_results`。
- KR 至少包含 `id`、`objective_id`、`owner`、`content`。

OAR 映射：

| Lark 字段 | OAR 领域模型 |
| --- | --- |
| `cycle_id` | `OkrCycle.id` |
| `objectives[].id` | `Objective.id` |
| `objectives[].owner.user_id` | `Objective.owner_user_id` |
| `key_results[].id` | `KeyResult.id` |
| `key_results[].objective_id` | `KeyResult.objective_id` |
| `content` | `Evidence.raw_content_ref` / 解析后的摘要 |

### T5：读取进展记录

目的：确认能读取 Objective / KR 的进展记录。

```bash
lark-cli okr +progress-list \
  --target-id "$OAR_TEST_KEY_RESULT_ID" \
  --target-type key_result \
  --format json
```

通过标准：

- 返回 `progress` 数组或空数组。
- 记录中包含 `progress_id`、`modify_time`、`content`、`progress_rate`。

### T6：创建进展 dry-run

目的：确认写回请求可以被预览，且 OAR 能把智能体意图转成受控工具请求。

```bash
lark-cli okr +progress-create \
  --target-id "$OAR_TEST_KEY_RESULT_ID" \
  --target-type key_result \
  --progress-percent 1 \
  --progress-status normal \
  --content '{"blocks":[{"block_element_type":"paragraph","paragraph":{"elements":[{"paragraph_element_type":"textRun","text_run":{"text":"OAR 阶段 0.5 dry-run 验证。请勿视为真实业务进展。"}}]}}]}' \
  --source-title "OAR 验证 dry-run" \
  --dry-run \
  --format json
```

通过标准：

- CLI 返回将要调用的 API、请求 payload、target、所需 scope。
- 不产生真实 progress record。

### T7：执行创建进展

目的：确认最小写回闭环可用。

执行前人工确认：

- 目标是一次性测试 KR。
- 内容明确标注为验证用途。
- scope 为 `okr:okr.content:writeonly`。
- 当前身份是测试用户。

```bash
lark-cli okr +progress-create \
  --target-id "$OAR_TEST_KEY_RESULT_ID" \
  --target-type key_result \
  --progress-percent 1 \
  --progress-status normal \
  --content '{"blocks":[{"block_element_type":"paragraph","paragraph":{"elements":[{"paragraph_element_type":"textRun","text_run":{"text":"OAR 阶段 0.5 验证进展。测试完成后可删除。"}}]}}]}' \
  --source-title "OAR 验证" \
  --format json
```

通过标准：

- 返回 `progress.progress_id`。
- 飞书 OKR 界面可看到该 progress record。
- `+progress-list` 能读取到新记录。

### T8：更新进展 dry-run / 执行

目的：确认 OAR 能修改自己创建的 progress record。

```bash
lark-cli okr +progress-update \
  --progress-id "$OAR_TEST_PROGRESS_ID" \
  --progress-percent 2 \
  --progress-status normal \
  --content '{"blocks":[{"block_element_type":"paragraph","paragraph":{"elements":[{"paragraph_element_type":"textRun","text_run":{"text":"OAR 阶段 0.5 验证进展已更新。"}}]}}]}' \
  --dry-run \
  --format json
```

通过 dry-run 后，再人工确认是否执行去掉 `--dry-run` 的命令。

通过标准：

- dry-run 可预览 payload。
- 执行后 `modify_time` 或内容发生变化。

### T9：删除进展 dry-run

目的：确认删除能力存在，但默认不启用生产执行。

```bash
lark-cli okr +progress-delete \
  --progress-id "$OAR_TEST_PROGRESS_ID" \
  --dry-run \
  --format json
```

执行策略：

- dry-run 可做。
- 执行只允许测试环境手动执行。
- OAR MVP 默认不开放删除能力。

## 7. 错误记录表

| 测试 ID | 命令 | 错误码 | 错误信息 | 缺失 scope | 身份 | 决策 |
| --- | --- | --- | --- | --- | --- | --- |
| T0 |  |  |  |  |  |  |
| T1 |  |  |  |  |  |  |
| T2 |  |  |  |  |  |  |
| T3 |  |  |  |  |  |  |
| T4 |  |  |  |  |  |  |
| T5 |  |  |  |  |  |  |
| T6 |  |  |  |  |  |  |
| T7 |  |  |  |  |  |  |
| T8 |  |  |  |  |  |  |
| T9 |  |  |  |  |  |  |

## 8. 通过 / 不通过标准

通过条件：

- `T0-T5` 全部通过。
- 至少 `T6` dry-run 通过。
- `T7` 在测试 KR 上执行成功，且能被 `T5` 读回。
- 缺权限错误可被稳定映射为缺失 scope、缺失资源权限或身份错误。
- CLI 输出可被解析成稳定 JSON。

有条件通过：

- 读取全部通过，但写入 scope 不可用。
- OAR 进入先读后写 MVP：每周简报、风险队列、证据链、任务/提醒/评论草稿。
- OKR progress 写回延后。

不通过条件：

- 无法稳定读取 OKR 周期和 Objective / KR。
- 无法确认当前执行身份。
- CLI 输出不稳定，无法解析成领域模型。
- 错误码无法区分缺 scope、缺资源权限、目标不存在。
- 写操作无法 dry-run。

## 9. 对 `LarkAdapter` 的要求

验证通过后，`LarkAdapter` 至少需要提供：

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

## 10. 参考来源

- Lark CLI GitHub：https://github.com/larksuite/cli
- Lark CLI `lark-okr` skill：https://github.com/larksuite/cli/blob/main/skills/lark-okr/SKILL.md
- `okr +cycle-list`：https://github.com/larksuite/cli/blob/main/skills/lark-okr/references/lark-okr-cycle-list.md
- `okr +cycle-detail`：https://github.com/larksuite/cli/blob/main/skills/lark-okr/references/lark-okr-cycle-detail.md
- `okr +progress-list`：https://github.com/larksuite/cli/blob/main/skills/lark-okr/references/lark-okr-progress-list.md
- `okr +progress-create`：https://github.com/larksuite/cli/blob/main/skills/lark-okr/references/lark-okr-progress-create.md
- `okr +progress-update`：https://github.com/larksuite/cli/blob/main/skills/lark-okr/references/lark-okr-progress-update.md
- `okr +progress-delete`：https://github.com/larksuite/cli/blob/main/skills/lark-okr/references/lark-okr-progress-delete.md
