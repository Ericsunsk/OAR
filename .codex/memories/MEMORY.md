# OAR Project Memory

Updated: 2026-05-26

## Core Identity

OAR is an OKR review cockpit for Lark/Feishu enterprise tenants. It helps teams run weekly OKR operations by finding execution risks, gathering evidence, proposing actions, and writing back to Lark only after user confirmation.

The central product thesis is that the main OKR opportunity is not helping people create goals, but helping teams operate goal execution every week.

## Product Positioning

OAR should stay narrow:

- Not a generic OKR SaaS.
- Not a Lark OKR replacement.
- Not a performance management or HR evaluation product.
- Not a generic agent desktop.
- Not an autonomous company-management agent.

The first version should be a weekly OKR review inbox. The user should be able to open OAR once a week and process OKR risks and pending actions in about 10 minutes.

Long-term, OAR can become an AI chief-of-staff layer and agent gateway for Lark OKR, but only if the weekly review loop works.

## Target Users

Primary ICP:

- 20-300 person teams.
- Heavy Lark/Feishu users.
- Already using OKRs.
- OKR execution operations are still handled manually.

First user types:

- Founder or CEO who wants company goal risks before weekly meetings.
- Manager or team lead who wants to know which KRs are blocked and who needs follow-up.
- PMO or chief of staff who wants to turn OKR reviews, reminders, and action follow-up into an inbox workflow.

## MVP Scope

MVP must include:

- Read OKR cycles, Objectives, Key Results, and progress from Lark.
- Detect stale KRs, low-progress KRs, and owners missing updates.
- Gather evidence summaries from Docs, Tasks, Meetings, Minutes, Calendar, and IM.
- Generate weekly briefs, risk queues, and suggested actions.
- Let users confirm, edit then confirm, or reject each suggestion.
- Write back only confirmed progress, comments, reminders, tasks, or meeting drafts.
- Record `AuditEvent` for actor, scope, target, before/after, and execution result.

MVP must not include:

- OKR creation.
- Performance evaluation.
- Automatic batch writeback.
- Generic agent marketplace.
- External A2A writeback to Lark.
- Automatic Objective creation/deletion.
- Automatic KR target, weight, owner, or cycle changes.
- Automatic progress deletion.

## Product Experience

Default entry is the review inbox, not a chat box and not a metrics dashboard.

Core weekly workflow:

1. Backend runtime syncs OKRs, tasks, meetings, docs, and progress.
2. OAR detects stale KRs, low-progress KRs, and owners missing updates.
3. OAR generates a risk queue, evidence chain, and suggested actions.
4. User opens the review inbox.
5. User reviews evidence and confirms, edits then confirms, or rejects actions.
6. Backend writes confirmed actions through `LarkAdapter`.
7. Audit timeline records who confirmed what, based on which evidence, and what was executed.

Desktop should feel like an OKR review cockpit. iOS is a lightweight approval and reminder surface. Lark entry points are important for bot notifications, cards, shortcuts, and confirmations.

Chat is only an auxiliary explanation and adjustment layer. It must not become the main workflow.

## Technical Direction

Preferred architecture:

- macOS: SwiftUI + AppKit bridge.
- iOS: SwiftUI companion and approval surface.
- Backend/core: Rust service.
- Integration: `LarkAdapter` wrapping Lark CLI and OpenAPI fallback paths.
- Storage: Postgres plus object storage plus vector index when needed.
- Runtime: server-side 7x24 scheduling, sync, audit, and tool execution.

Important system boundary:

- Lark is the authority for OKRs, Docs, Tasks, Meetings, Calendar, and IM raw data.
- OAR backend is the authority for reviews, pending actions, audit events, evidence indexes, memory, and sync cursors.
- Clients are for interaction, viewing, approval, local UI cache, and drafts.

## Current Validation State

Current stage is Phase 0.6: identity and sync validation.

Phase 0.5 is complete:

- `lark-okr` is validated as the OKR read path.
- `lark-okr` is validated for progress create and update writeback.
- Progress delete is dry-run only and must not be a default MVP write capability.
- CLI validation version was `1.0.39`.

Phase 0.6 first checks are complete:

- user and bot identities can be verified.
- default execution identity is user.
- `offline_access` is granted.
- `auth:user.id:read` is granted.
- token validity and expiry metadata are visible.
- CLI did not expose access or refresh tokens in tested output.

Phase 0.6 still needs backend implementation and verification:

- encrypted `TokenGrant` storage.
- refresh token rotation with atomic persistence.
- revoke and reauth behavior.
- `OperationLedger` idempotent execution.
- multi-device state sync.
- background worker behavior.
- complete `AuditEvent` traceability.

## Safety Model

Default safety principle:

> Read first, dry-run before write, human confirmation before execution.

Human users remain responsible for goal judgment, authorization, organizational commitment, and final writeback decisions. Agents may observe, diagnose, draft, explain, and propose.

All writebacks must come from `ConfirmedAction`.

Never allow:

- LLM direct raw command execution.
- LLM-driven scope escalation.
- Unconfirmed group messages, OKR edits, meeting creation, or batch task changes.
- External A2A agents receiving identity tokens or raw CLI stdout/stderr.
- Logs containing access tokens, refresh tokens, full meeting transcripts, sensitive HR conclusions, or raw cross-team data.

## Phase 0.6 Priority

The next engineering priority is not UI. Build the identity, sync, idempotency, and audit skeleton first.

Recommended next work:

1. Define Rust domain models for `Tenant`, `OarUser`, `LarkIdentity`, `TokenGrant`, `DeviceSession`, `OperationLedger`, and `AuditEvent`.
2. Save Phase 0.5 Lark CLI outputs as `LarkAdapter` fixtures.
3. Implement parsers for validated CLI outputs and quirks.
4. Implement `ConfirmedAction -> OperationLedger -> LarkAdapter -> AuditEvent`.
5. Add concurrency tests proving the same `ConfirmedAction` executes only once.

## Known Lark CLI Quirks

Parser and fixture work must account for:

- `auth status` returns JSON but does not support `--format json`.
- `auth check` needs `--scope`.
- dry-run output may be prefixed with `=== Dry Run ===`.
- `cycle-detail` Objective/KR `content` is a JSON string and needs a second parse.
- `progress-list` uses `data.progress_list[]`, not `data.progress[]`.
- create/update responses use string status values such as `normal`; dry-run payload status may be numeric.
- `okr.progress_records.*` native schema was not available in the validated CLI.

## Memory Architecture

OAR should use a three-layer memory model:

- Ontology Graph: stable entities and relationships such as tenant, user, team, OKR cycle, Objective, KeyResult, task, document, meeting, evidence, action, and feedback.
- Vector Memory: semantic retrieval over summaries from docs, meetings, tasks, OKR progress, reviews, and feedback.
- Decision Memory: confirmations, edits, rejections, preferences, trust calibration, and user/team decision patterns.

Memory is not evidence. It can help retrieve and rank context, but writeback must trace to current evidence, user confirmation, execution identity, scope, and audit result.

MVP memory should use Postgres tables, relation tables, and possibly pgvector. Avoid complex graph databases, raw full-message indexing, cross-tenant memory, and hidden personal profiling.

## Success Signals

OAR is worth continuing if a real team uses it for 2-4 weeks and shows:

- Review preparation time reduced by about 50%.
- Confirm or edit-then-confirm rate reaches 30% or more.
- Users can explain why they trust or distrust the evidence chain.
- At least one active weekly review session.
- 100% of writes have confirmation records and audit events.

Stop or rethink if users do not return weekly, suggestions stay below 10% confirmation, evidence cannot be trusted, permissions cannot be safely handled, or enterprises prefer to keep the whole workflow inside Lark without a separate cockpit.
