# Feishu Calendar

## Domain

Read-only summaries of the current user's Feishu primary calendar free/busy state and limited event agenda.

## Activation

Activate only when the latest user request asks to read, show, count, or summarize the user's own Feishu calendar free/busy, availability, agenda, event list, schedule, or meetings.

Do not activate for creating meetings, updating events, inviting attendees, booking rooms, notifying people, team/department availability, colleague availability, or another person's calendar.

## Tool Bindings

- `feishu.calendar.summarize_my_free_busy`: read-only summary of the current user's primary-calendar busy windows for the next 7 days.
- `feishu.calendar.summarize_my_events`: read-only limited summary of the current user's primary-calendar event instances for the next 7 days.

## Safety

This skill describes domain capability, activation conditions, and backend tool IDs only. It does not execute platform operations. Runtime reads must go through the backend tool runtime and Lark adapter after OAuth scope checks.

The free-busy tool returns a compact busy-window summary, not event titles, event descriptions, attendee lists, raw payloads, or full calendar records.

The event summary tool returns only a bounded agenda summary: total count and up to 5 examples with start/end, title, location name, organizer display name, status, and free/busy. It must not return descriptions, meeting URLs, app links, raw IDs, attachments, or full attendee lists.
