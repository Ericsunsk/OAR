# Feishu OKR

## Domain

Read-only summaries of the current user's Feishu OKR cycles, objectives, key results, counts, and progress signals.

## Activation

Activate only when the latest user request asks to read, show, list, or count the user's own OKR data, or asks about the user's own OKR progress, stale status, risk, delay, update records, or latest updates.
Contextual Feishu count questions may activate only when recent conversation or workspace context is already about OKR.

Do not activate for team, department, colleague, or other-person OKR requests. Do not activate for generic business goals such as target customers.
Do not activate for OKR progress write intents such as update, set, write, delete, submit, post, comment, remind, 新增, 创建, 删除, 提交, 发布, 评论, or 提醒.

## Tool Bindings

- `feishu.okr.summarize_my_okr`: read-only summary of the current user's Feishu OKR cycles, objectives, and KR counts.
- `feishu.okr.summarize_my_progress`: read-only summary of the current user's Feishu OKR progress, latest updates, stale/delay status, and risk signals.

## Safety

This skill describes domain capability, activation conditions, and backend tool IDs only. It does not execute platform operations. Runtime reads must go through the backend tool runtime and Lark adapter after OAuth scope checks.
