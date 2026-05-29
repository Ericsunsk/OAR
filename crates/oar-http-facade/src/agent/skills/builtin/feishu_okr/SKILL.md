# Feishu OKR

## Domain

Read-only summaries of the current user's Feishu OKR cycles, objectives, key results, and counts.

## Activation

Activate only when the latest user request asks to read, show, list, or count the user's own OKR data.
Contextual Feishu count questions may activate only when recent conversation or workspace context is already about OKR.

Do not activate for team, department, colleague, or other-person OKR requests. Do not activate for generic business goals such as target customers.

## Tool Bindings

- `feishu.okr.summarize_my_okr`: read-only summary of the current user's Feishu OKR cycles, objectives, and KR counts.

## Safety

This skill describes domain capability, activation conditions, and backend tool IDs only. It does not execute platform operations. Runtime reads must go through the backend tool runtime and Lark adapter after OAuth scope checks.
