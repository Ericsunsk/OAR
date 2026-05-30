import Foundation

/// Client-facing read model expected from the OAR backend.
///
/// This intentionally carries only normalized display data, evidence summaries,
/// hashes, and audit summaries. It must never include platform tokens, raw
/// meeting transcripts, full document bodies, or unsanitized adapter payloads.
struct ReviewInboxAPISnapshot: Codable, Equatable {
    let contractVersion: Int
    let generatedAt: String
    let items: [ReviewInboxItemDTO]
    let proposedActions: [ProposedActionDTO]
    let evidence: [EvidenceItemDTO]
    let ledgerEvents: [LedgerEventDTO]

    enum CodingKeys: String, CodingKey {
        case contractVersion = "contract_version"
        case generatedAt = "generated_at"
        case items
        case proposedActions = "proposed_actions"
        case evidence
        case ledgerEvents = "ledger_events"
    }
}

struct ReviewInboxItemDTO: Codable, Equatable, Identifiable {
    let id: String
    let tenantID: String
    let userID: String
    let proposedActionID: String
    let proposedActionVersion: UInt64
    let objectiveTitle: String
    let keyResultTitle: String
    let ownerDisplayName: String
    let weekLabel: String
    let riskScore: UInt32
    let priority: UInt32
    let riskReason: String
    let confidenceScore: Double
    let status: ReviewInboxItemStatusDTO
    let syncCursor: UInt64
    let updatedAtDisplay: String
    let ledgerStatus: String?
    let operationID: String?

    enum CodingKeys: String, CodingKey {
        case id
        case tenantID = "tenant_id"
        case userID = "user_id"
        case proposedActionID = "proposed_action_id"
        case proposedActionVersion = "proposed_action_version"
        case objectiveTitle = "objective_title"
        case keyResultTitle = "key_result_title"
        case ownerDisplayName = "owner_display_name"
        case weekLabel = "week_label"
        case riskScore = "risk_score"
        case priority
        case riskReason = "risk_reason"
        case confidenceScore = "confidence_score"
        case status
        case syncCursor = "sync_cursor"
        case updatedAtDisplay = "updated_at_display"
        case ledgerStatus = "ledger_status"
        case operationID = "operation_id"
    }
}

enum ReviewInboxItemStatusDTO: String, Codable {
    case open
    case confirmed
    case rejected
    case executing
    case succeeded
    case failed
    case withdrawn
}

struct ProposedActionDTO: Codable, Equatable, Identifiable {
    let id: String
    let reviewItemID: String
    let tenantID: String
    let actorUserID: String
    let targetUserID: String?
    let ownerUserID: String?
    let version: UInt64
    let status: ProposedActionStatusDTO
    let kind: ProposedActionKindDTO
    let riskSeverity: RiskSeverityDTO
    let evidenceIDs: [String]
    let rationale: String
    let expectedImpact: String
    let dryRunResultSummary: String
    let estimatedWriteTargetsCount: Int
    let decision: ProposedActionDecisionDTO?

    enum CodingKeys: String, CodingKey {
        case id
        case reviewItemID = "review_item_id"
        case tenantID = "tenant_id"
        case actorUserID = "actor_user_id"
        case targetUserID = "target_user_id"
        case ownerUserID = "owner_user_id"
        case version
        case status
        case kind
        case riskSeverity = "risk_severity"
        case evidenceIDs = "evidence_ids"
        case rationale
        case expectedImpact = "expected_impact"
        case dryRunResultSummary = "dry_run_result_summary"
        case estimatedWriteTargetsCount = "estimated_write_targets_count"
        case decision
    }
}

enum ProposedActionStatusDTO: String, Codable {
    case draft
    case published
    case superseded
    case withdrawn
}

enum ProposedActionKindDTO: String, Codable {
    case createKrProgress = "create_kr_progress"
    case updateKrProgress = "update_kr_progress"
    case deleteKrProgressDryRun = "delete_kr_progress_dry_run"
    case pingOwner = "ping_owner"
    case createTask = "create_task"
    case scheduleReview = "schedule_review"
    case custom
}

enum RiskSeverityDTO: String, Codable {
    case low
    case medium
    case high
    case critical
}

enum ProposedActionDecisionDTO: String, Codable {
    case confirm
    case editThenConfirm = "edit_then_confirm"
    case reject
}

struct EvidenceItemDTO: Codable, Equatable, Identifiable {
    let id: String
    let reviewItemID: String
    let sourceKind: EvidenceSourceKindDTO
    let sourceID: String
    let locator: String?
    let observedAtDisplay: String
    let summary: String
    let signalType: SignalTypeDTO
    let trustScore: Double
    let contentHash: String
    let visibility: EvidenceVisibilityDTO

    enum CodingKeys: String, CodingKey {
        case id
        case reviewItemID = "review_item_id"
        case sourceKind = "source_kind"
        case sourceID = "source_id"
        case locator
        case observedAtDisplay = "observed_at_display"
        case summary
        case signalType = "signal_type"
        case trustScore = "trust_score"
        case contentHash = "content_hash"
        case visibility
    }
}

enum EvidenceSourceKindDTO: String, Codable {
    case okrProgress = "okr_progress"
    case larkMinutes = "lark_minutes"
    case larkDoc = "lark_doc"
    case larkTask = "lark_task"
    case larkCalendar = "lark_calendar"
    case larkIM = "lark_im"
    case manualReviewNote = "manual_review_note"
    case auditEvent = "audit_event"
}

enum EvidenceVisibilityDTO: String, Codable {
    case tenant
    case team
    case user
}

enum SignalTypeDTO: String, Codable {
    case progress
    case blocker
    case dependency
    case cadence
}

struct LedgerEventDTO: Codable, Equatable, Identifiable {
    let id: String
    let actionID: String
    let stage: LedgerStageDTO
    let stageStatus: LedgerStatusDTO
    let timestampDisplay: String
    let message: String
    let idempotencyKey: String

    enum CodingKeys: String, CodingKey {
        case id
        case actionID = "action_id"
        case stage
        case stageStatus = "stage_status"
        case timestampDisplay = "timestamp_display"
        case message
        case idempotencyKey = "idempotency_key"
    }
}

enum LedgerStageDTO: String, Codable {
    case confirmedAction = "confirmed_action"
    case operationLedger = "operation_ledger"
    case platformAdapter = "platform_adapter"
    case auditEvent = "audit_event"
}

enum LedgerStatusDTO: String, Codable {
    case pending
    case ok
    case error
}

struct ReviewDecisionDTO: Codable, Equatable {
    let actionID: String
    let actionVersion: UInt64
    let decision: ProposedActionDecisionDTO
    let note: String
    let expectedSyncCursor: UInt64

    enum CodingKeys: String, CodingKey {
        case actionID = "action_id"
        case actionVersion = "action_version"
        case decision
        case note
        case expectedSyncCursor = "expected_sync_cursor"
    }
}
