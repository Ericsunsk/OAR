# EIA Memory Principle

EIA separates authority, decision, and memory.

Host platforms own raw facts. The Agent backend owns decisions, confirmations, ledgers, and audit trails. Memory helps rank, recall, and suggest; it never proves current truth.

Every physical write must be based on freshly queried live state, validated by dry-run, explicitly confirmed by a human, executed once through an idempotent ledger, and recorded as an immutable audit event.

## Operating Mantra

Memory suggests. Live state proves. Humans confirm. Ledgers execute. Audits remember.

## Preserved Architecture Decisions

- Authority is asymmetric: third-party platforms remain the system of record; OAR remains the system of decision.
- Confirmed writes flow through `ConfirmedAction -> OperationLedger -> PlatformAdapter -> AuditEvent`.
- `OperationLedger` enforces exactly-once execution by unique `ActionID` using database constraints or distributed locks.
- `AuditEvent` is append-only and must not contain credentials, tokens, auth codes, full transcripts, or sensitive raw payloads.
- Memory has three logical layers: ontology for relational facts, vector memory for semantic recall, and decision memory for user feedback patterns.
- Platform quirks belong in parser and adapter layers. Domain logic consumes normalized models.
- Real and anomalous platform responses should be captured as local fixtures for offline regression tests.
