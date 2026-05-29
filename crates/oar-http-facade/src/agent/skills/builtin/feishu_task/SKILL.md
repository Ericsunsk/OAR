# Feishu Task

## Domain

Read-only summaries of the current user's Feishu tasks, especially the "my_tasks" list that represents tasks assigned to the user.

## Activation

Activate only when the latest user request asks to read, show, list, count, or summarize the user's own Feishu tasks or todos.

Do not activate for creating, updating, deleting, assigning, or commenting on tasks. Do not activate for team, department, colleague, or other-person task requests.

## Tool Bindings

- `feishu.task.summarize_my_tasks`: read-only summary of the current user's Feishu "my_tasks" task list.

## Safety

This skill describes domain capability, activation conditions, and backend tool IDs only. It does not execute platform operations. Runtime reads must go through the backend tool runtime and Lark adapter after OAuth scope checks.
