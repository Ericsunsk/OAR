# Feishu Minutes

## Domain

Read-only summaries of the current user's Feishu Minutes / meeting notes list.

## Activation

Activate only when the latest user request asks to read, show, list, count, or summarize the user's own Feishu Minutes, recent Minutes, meeting notes, or meeting-note metadata.

Do not activate for team, department, colleague, or another person's Minutes. Do not activate for uploading, deleting, sharing, exporting media, exporting transcripts, or reading full raw transcripts.

## Tool Bindings

- `feishu.minutes.summarize_my_minutes`: read-only summary of the current user's Feishu Minutes / meeting notes count and safe metadata examples.

## Safety

This skill describes domain capability, activation conditions, and backend tool IDs only. It does not execute platform operations. Runtime reads must go through the backend tool runtime and Lark adapter after OAuth scope checks.

The summary tool may return only bounded metadata such as total count, title examples, creation time, and duration. It must not return raw minute tokens, source URLs, owner IDs, cover/media links, full transcripts, artifacts, raw payloads, or attendee/member records.
