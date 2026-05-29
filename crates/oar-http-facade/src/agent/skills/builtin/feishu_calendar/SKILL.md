# Feishu Calendar

## Domain

Read-only summaries of the current user's Feishu calendar free/busy state.

## Activation

Activate only when the latest user request asks to read, show, count, or summarize the user's own Feishu calendar free/busy or availability.

Do not activate for creating meetings, updating events, inviting attendees, booking rooms, notifying people, team/department availability, colleague availability, or generic calendar event listing.

## Tool Bindings

- `feishu.calendar.summarize_my_free_busy`: read-only summary of the current user's primary-calendar busy windows for the next 7 days.

## Safety

This skill describes domain capability, activation conditions, and backend tool IDs only. It does not execute platform operations. Runtime reads must go through the backend tool runtime and Lark adapter after OAuth scope checks.

The tool returns a compact busy-window summary, not event titles, event descriptions, attendee lists, raw payloads, or full calendar records.
