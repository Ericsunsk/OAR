# OAR macOS Review Inbox

SwiftUI macOS client for the OAR review inbox.

This app is currently a production-facing shell, not a production writeback client. The default provider is still `MockReviewInboxDataProvider`, and every real platform write must remain server-side.

## Current Shape

- Three-column macOS layout: navigation, review workspace, OAR Agent sidecar.
- Glass visual treatment with native toolbar placement.
- Async `ReviewInboxDataProviding` boundary for mock or remote data.
- API DTO contract in `ReviewInboxAPIContract.swift`, mapped into display models.
- View model tests for load, approve, reject, and capability boundary behavior.

## Source Layout

```text
Sources/OARReviewInbox/
  App/                         App entry and window shell
  Design/                      Shared colors, fonts, and small design primitives
  Features/ReviewInbox/
    Domain/                    Client-side display models and filter enums
    Data/                      API DTOs, providers, and mock fixtures
    Presentation/              SwiftUI views and view models
Tests/OARReviewInboxTests/
  Features/ReviewInbox/        Feature-level contract and view-model tests
```

Naming rule: API payload types keep the `DTO` suffix and mirror backend contracts; client display models use the `ReviewInboxDisplay*` prefix; data entry points use `ReviewInboxData*`.

## Production Boundary

The frontend may display:

- `ReviewInboxItem` backend read models mapped to `ReviewInboxDisplayItem`.
- `ProposedAction` summaries and dry-run summaries.
- `Evidence` summaries, references, visibility, and content hashes.
- `OperationLedger` / `AuditEvent` summaries for the timeline.

The frontend must not receive or display:

- Lark or OAR credentials, auth codes, access tokens, or refresh tokens.
- Raw meeting transcripts, full document bodies, or unsanitized adapter payloads.
- Any platform-owned master-field mutation controls such as owner, target, weight, or OKR cycle edits.

## Execution Rule

The client can request a decision, but real execution must be enforced by the backend chain:

`ConfirmedAction -> OperationLedger -> PlatformAdapter -> AuditEvent`

For MVP, only KR progress create/update should enter production execution. Other suggested actions can be shown as drafts or rejected, but must not write to Feishu/Lark from the client.

## Local Verification

```bash
swift build
swift test
```

If `swift test` cannot write the user-level Swift or clang cache in a sandboxed session, rerun it with the appropriate local permissions.
