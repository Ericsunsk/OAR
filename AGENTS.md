# AGENTS.md

This repository follows the Enterprise Integration Agent (EIA) paradigm.

## Core Boundary

Host platforms such as Lark, Slack, Teams, Salesforce, calendars, and document systems are the source of truth for raw business data.

The Agent backend is the source of truth for analysis, proposed actions, confirmations, operation ledgers, audit events, and decision state.

Memory is context, not evidence. Before any physical write, always re-read live platform state.

## Non-Negotiable Safety Rules

1. Read first.
2. Dry-run before write.
3. Human confirmation before execution.
4. Every confirmed write must pass through `ConfirmedAction -> OperationLedger -> PlatformAdapter -> AuditEvent`.
5. Writes must be idempotent and exactly-once by `ActionID`.
6. Never expose credentials, tokens, auth codes, sensitive raw payloads, or full unredacted transcripts.
7. Never perform unsupervised batch writes, destructive deletes, or direct changes to platform-owned master fields.

## Engineering Rules

Production integrations must use platform adapters or SDK clients, not ad-hoc platform CLI calls.

CLI tools may only be used for local validation, fixture recording, debugging, or regression testing.

Platform quirks must be isolated in parser or adapter layers. Business logic should consume normalized domain models.

Maintain fixture-driven tests for real and anomalous platform responses.

Make narrow, testable changes. Preserve historical decisions and documentation unless merging duplicate material.
