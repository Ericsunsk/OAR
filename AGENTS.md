# AGENTS.md

This file gives project-level instructions for AI agents working in this repository.

## Project Snapshot

OAR is an OKR review cockpit for Lark/Feishu enterprise tenants. It helps teams run weekly OKR operations by finding execution risks, gathering evidence, proposing actions, and writing back to Lark only after user confirmation.

The product is not a generic OKR SaaS, not a Lark OKR replacement, not a performance review system, and not a generic agent desktop. Keep the first version narrow: a weekly OKR review inbox.

Current stage:

- Phase 0.5 is complete. `lark-okr` is validated as the OKR read path and the progress create/update write path. Progress delete is dry-run only for MVP.
- Phase 0.6 is in progress. The current engineering focus is identity, authorization, token refresh, multi-device sync, idempotent execution, and auditability.

## Source Of Truth

Start with these documents, in this order:

1. `docs/one-page-brief.md`
2. `docs/product-plan.md`
3. `docs/technical-architecture.md`
4. `docs/security-and-permissions.md`
5. `docs/validation-plan.md`
6. `docs/phase-0.5-lark-cli-validation-report.md`
7. `docs/phase-0.6-identity-sync-validation-report.md`

Use `docs/memory-architecture.md` when working on retrieval, memory, evidence, decision feedback, or long-term learning.

## Product Principles

- Default entry is the review inbox, not a chat box or dashboard.
- The weekly workflow is: sync evidence, detect OKR risks, generate suggestions, let users confirm/edit/reject, then write back and audit.
- Every suggested action must have an evidence chain.
- `Confirm`, `edit then confirm`, and `reject` are first-class actions.
- User trust matters more than automation speed.
- If users do not return weekly, adding A2A, agents, dashboards, or complex architecture will not save the product.

## Safety And Permission Rules

- Default security model: read first, dry-run before write, human confirmation before execution.
- All writebacks must come from a `ConfirmedAction`.
- Never let an LLM execute raw shell, arbitrary CLI commands, or unreviewed OpenAPI calls.
- All Lark calls must go through `LarkAdapter` or a deliberately designed adapter layer.
- Do not log or expose access tokens, refresh tokens, authorization codes, full meeting transcripts, sensitive HR judgments, or raw cross-team data.
- External A2A agents must never receive identity tokens, raw CLI stdout/stderr, raw OKR data, raw memory, or direct write access.
- MVP must not automatically create/delete Objectives, modify KR target/weight/owner/cycle, delete progress, evaluate personal performance, or batch-write organizational changes.

## Engineering Direction

Preferred technical shape:

- macOS: SwiftUI + AppKit bridge.
- iOS: SwiftUI approval and companion surface only.
- Backend/core: Rust service.
- Integration: `LarkAdapter` wrapping Lark CLI and OpenAPI fallback paths.
- Storage: Postgres plus object storage plus vector index when needed.
- Runtime: server-side scheduling, sync, audit, and tool execution.

Phase 0.6 should be implemented before investing heavily in UI:

- Define `Tenant`, `OarUser`, `LarkIdentity`, `TokenGrant`, `DeviceSession`, `OperationLedger`, and `AuditEvent`.
- Save Phase 0.5 CLI outputs as `LarkAdapter` fixtures.
- Implement parser tests for CLI quirks.
- Build the minimal state machine: `ConfirmedAction -> OperationLedger -> LarkAdapter -> AuditEvent`.
- Add local concurrency tests proving the same `ConfirmedAction` can only execute once.

## Lark CLI Notes

Validated CLI behavior to preserve in fixtures and parsers:

- Installed CLI version during validation: `1.0.39`.
- `auth status` returns JSON but does not support `--format json`.
- `auth check` uses `--scope`.
- Dry-run output may include a `=== Dry Run ===` prefix.
- `cycle-detail` returns Objective/KR `content` as a JSON string that needs a second parse.
- `progress-list` returns `data.progress_list[]`, not `data.progress[]`.
- create/update responses may use string status values while dry-run payloads may use numeric status values.
- `okr.progress_records.*` native schemas were not available in the validated CLI; progress support relies on shortcuts and dry-run validation.

## Memory And Evidence

OAR memory should follow a three-layer model:

- Ontology graph for entities and relationships.
- Vector memory for semantic retrieval over summaries and evidence.
- Decision memory for user confirmations, edits, rejections, preferences, and trust calibration.

Memory is not evidence. Memory may improve retrieval and ranking, but writeback decisions must trace back to current evidence, user confirmation, execution identity, scope, and audit events.

For MVP, prefer Postgres tables and relation tables over a graph database. Store summaries, references, hashes, and visibility scopes before storing raw content.

## Coding Guidelines For This Repo

- Keep changes narrow and aligned with the current phase.
- Prefer small, testable Rust modules for domain state, adapters, parsers, and audit logic.
- Keep CLI/OpenAPI parsing isolated behind adapters.
- Add fixtures and regression tests whenever encoding known Lark CLI behavior.
- Do not introduce broad platform abstractions until Phase 0.6 state and audit semantics are proven.
- If adding frontend code, preserve the review inbox as the main surface and avoid making chat the primary workflow.

## Git And Documentation Hygiene

- Do not remove or rewrite project decisions without updating the relevant source document.
- Keep README as an entry point and do not overload it with implementation detail.
- Do not commit real tenant IDs, user IDs, object IDs, tokens, authorization codes, or sensitive raw outputs.
- If a doc becomes redundant, merge the surviving information into the appropriate source-of-truth doc before deletion.
