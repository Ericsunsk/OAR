# OAR macOS

SwiftUI macOS client for OAR.

This app is a production-facing review client, not a production writeback client. By default it requires an OAR backend and never falls back to mock review data or a mock login session unless an explicit local-development flag is enabled.

## Current Shape

- Three-column macOS layout: navigation, review workspace, OAR Agent sidecar.
- Glass visual treatment with native toolbar placement.
- Async `ReviewInboxDataProviding` boundary for OAR backend data.
- API DTO contract in `ReviewInboxAPIContract.swift`, mapped into display models.
- View model tests for load, approve, reject, and capability boundary behavior.

## Source Layout

```text
Sources/OAR/
  App/                         App entry and window shell
  Design/                      Shared colors, fonts, and small design primitives
  Features/ReviewInbox/
    Domain/                    Client-side display models and filter enums
    Data/                      API DTOs, providers, and local-only mock fixtures
    Presentation/              SwiftUI views and view models
  Features/Agent/              Sidecar UI and OAR backend streaming provider
Tests/OARTests/
  Features/ReviewInbox/        Feature-level contract and view-model tests
```

Naming rule: API payload types keep the `DTO` suffix and mirror backend contracts; client display models use the `ReviewInboxDisplay*` prefix; data entry points use `ReviewInboxData*`.

## Runtime Configuration

The app connects to `http://127.0.0.1:8080` by default. Start the local backend
facade before launching the client:

```bash
# terminal 1
cargo run -p oar-http-facade

# terminal 2
swift run
```

Future private deployment support should use an in-app server setting rather
than process environment variables.

The frontend calls only OAR backend endpoints:

- `POST /auth/feishu/qr-sessions`
- `GET /auth/feishu/callback`
- `GET /auth/feishu/qr-sessions/{session_id}`
- `GET /auth/feishu/qr-sessions/{session_id}/events`
- `GET /review-inbox/snapshot`
- `POST /review-inbox/decisions`
- `POST /agent/stream`

Current repository status: `oar-http-facade` can create a real Feishu OAuth
authorization URL when `OAR_FEISHU_APP_ID`, `OAR_FEISHU_APP_SECRET`, and
`OAR_FEISHU_REDIRECT_URI` are configured. The callback exchanges the Feishu
authorization code server-side and returns only an OAR session plus safe user
display fields to the client. When the backend is configured with `DATABASE_URL`
and a grant encryption key, the callback stores an encrypted per-user
`TokenGrant`; the default Feishu login scope is `offline_access` so Feishu can
return a refresh token. Review Inbox live data is still backend follow-up work,
so the snapshot endpoint currently returns an empty Review Inbox contract and decision
write paths remain disabled until the
`ConfirmedAction -> OperationLedger -> PlatformAdapter -> AuditEvent` execution
chain is connected. `OAR_FEISHU_APP_ID` / `OAR_FEISHU_APP_SECRET` identify the
OAR Feishu application for this deployment, not a shared user or bot identity;
each successful scan creates a grant bound to the Feishu tenant and user. For
Docker, the backend may set
`OAR_HTTP_BIND_ADDR=0.0.0.0:8080`; the macOS client remains hardwired to the
local backend origin until in-app server settings are introduced.

The Agent sidecar never calls model APIs directly. It streams through
`/agent/stream` with the current OAR session. Users can enter `baseURL` and
`apiKey` in the Agent settings sheet; the backend detects the protocol/model
catalog and stores the API key encrypted. If no user BYOK setting exists, the
backend falls back to `OAR_AGENT_PROVIDER` plus either the `OAR_AGENT_OPENAI_*`
or `OAR_AGENT_ANTHROPIC_*` env group.

```bash
docker build -f ../../docker/backend.Dockerfile -t oar-backend ../..
docker run --rm -p 8080:8080 oar-backend
docker compose -f ../../docker/compose.dev.yml up --build
```

Mock fallbacks remain test-only injection paths and should not be exposed for
production validation.

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

The capability and permission matrix is maintained in
[`docs/agent-capabilities-feishu-permissions.md`](../../docs/agent-capabilities-feishu-permissions.md);
the macOS client should treat that document as the backend contract for which
actions may be displayed, confirmed, or kept as drafts.

## Local Verification

```bash
swift run
swift build
swift test
```

If `swift test` cannot write the user-level Swift or clang cache in a sandboxed session, rerun it with the appropriate local permissions.
