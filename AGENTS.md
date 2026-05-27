# AGENTS.md

This file gives project-level instructions for AI agents working in this repository.

## 1. Project Paradigm & Scope

*   **OAR**: A weekly OKR review cockpit (Review Inbox) for Lark/Feishu tenants. Focuses on **execution operations**, not creation.
*   **Current State**: Phase 0.5 (CLI validated fixtures & write baseline) is complete. Phase 0.6 (identity, auth refresh, multi-device sync, idempotent execution, auditability) is in progress.
*   **System Boundary**: Lark is the authority for raw tenant data (docs, tasks, calendar, IM). OAR is the authority for reviews, pending actions, audit events, and decisions.

---

## 2. Safety & Permission Rules

> **Core Principle**: Read first, dry-run before write, human confirmation before execution.

*   **All writebacks** must originate from a `ConfirmedAction` via `ConfirmedAction -> OperationLedger -> LarkAdapter -> AuditEvent`.
*   **LLMs must never**:
    *   Execute raw shell, arbitrary CLI, or unreviewed OpenAPI calls.
    *   Initiate automatic batch writes, OKR target/weight/owner edits, objective creation/deletion, or progress deletions.
    *   Expose or log access tokens, refresh tokens, auth codes, full meeting transcripts, or raw cross-team/sensitive HR data.
    *   Provide identity tokens or raw memory/CLI stdout to external A2A agents.

---

## 3. Engineering & Tech Stack

*   **Architecture**: macOS client (SwiftUI + AppKit bridge), iOS companion surface (SwiftUI), Rust backend core.
*   **Database**: Postgres (relational + pgvector) for the three-layer memory (Ontology, Vector, Decision). Avoid graph DBs.
*   **Integration**: Production path is `LarkAdapter` (Rust OpenAPI adapter). Lark CLI is strictly for validation, fixtures, and regression tests.
*   **Phase 0.6 Priority**: Focus on identity, sync, idempotency, and audit backend skeleton before investing in UI. Define: `Tenant`, `OarUser`, `LarkIdentity`, `TokenGrant`, `DeviceSession`, `OperationLedger`, `AuditEvent`.

---

## 4. Lark CLI Quirks (v1.0.39) Reference

Parsers and mock fixtures must handle these anomalies:
*   `auth status` returns JSON but rejects the `--format json` argument.
*   `auth check` requires `--scope`.
*   Dry-run output may contain a `=== Dry Run ===` prefix.
*   `cycle-detail` Objective/KR `content` is double-serialized (nested JSON string) requiring a second parse.
*   `progress-list` returns list in `data.progress_list[]`, not `data.progress[]`.
*   Dry-run payload uses numeric status; create/update APIs use string status.
*   `okr.progress_records.*` schemas are absent; progress operations use shortcuts and dry-run validation.

---

## 5. Coding & Hygiene Guidelines

*   **Changes**: Narrow scope, small testable Rust modules, isolated OpenAPI/CLI parsers.
*   **Tests**: Add fixtures and regression tests for known Lark CLI quirks. Use local concurrency tests to verify single-execution of `ConfirmedAction`.
*   **UI**: Keep the review inbox as the primary view. Avoid making chat the main workflow.
*   **Secrets**: Never commit real tenant IDs, user IDs, object IDs, tokens, or sensitive raw payloads.
*   **Docs**: Do not remove/rewrite decisions without updating source documents. Merge redundant information before deleting docs.
