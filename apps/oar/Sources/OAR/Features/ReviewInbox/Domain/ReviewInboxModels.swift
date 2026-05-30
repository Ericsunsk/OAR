import Foundation
import SwiftUI

enum ReviewInboxRiskLevel: String, CaseIterable, Identifiable {
    case low = "低"
    case medium = "中"
    case high = "高"
    case critical = "严重"

    var id: String { rawValue }

    var rank: Int {
        switch self {
        case .low: 1
        case .medium: 2
        case .high: 3
        case .critical: 4
        }
    }

    var color: Color {
        switch self {
        case .low: Color.oarMoss
        case .medium: Color.oarAmber
        case .high: Color.oarRust
        case .critical: Color.oarSignal
        }
    }
}

enum ReviewInboxDisplayStatus: String, CaseIterable, Identifiable {
    case new = "新风险"
    case needsConfirmation = "待确认"
    case confirmed = "已确认"
    case executing = "执行中"
    case executed = "已执行"
    case failed = "失败"
    case rejected = "已拒绝"
    case cancelled = "已取消"

    var id: String { rawValue }
}

enum ReviewInboxLedgerStatus: Equatable {
    case confirmed
    case executing
    case succeeded
    case failed
    case cancelled
    case unknown(String)

    init?(apiValue: String?) {
        guard let normalized = apiValue?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased(),
              !normalized.isEmpty else {
            return nil
        }

        switch normalized {
        case "confirmed":
            self = .confirmed
        case "executing":
            self = .executing
        case "succeeded":
            self = .succeeded
        case "failed":
            self = .failed
        case "cancelled", "canceled":
            self = .cancelled
        default:
            self = .unknown(normalized)
        }
    }

    var displayStatus: ReviewInboxDisplayStatus? {
        switch self {
        case .confirmed:
            return .confirmed
        case .executing:
            return .executing
        case .succeeded:
            return .executed
        case .failed:
            return .failed
        case .cancelled:
            return .cancelled
        case .unknown:
            return nil
        }
    }
}

enum ReviewInboxEvidenceSource: String {
    case okr = "OKR"
    case task = "任务"
    case calendar = "日历"
    case meeting = "会议"
    case doc = "文档"
    case im = "消息"
}

enum ReviewInboxSignal: String {
    case progress = "进展"
    case blocker = "阻塞"
    case dependency = "依赖"
    case cadence = "节奏"
}

enum ReviewInboxActionType: String {
    case updateProgress = "更新进展"
    case pingOwner = "提醒负责人"
    case createTask = "创建任务"
    case scheduleReview = "安排复盘"
}

enum ReviewInboxGateState: String {
    case pending = "待处理"
    case approved = "已确认"
    case rejected = "已拒绝"
    case draft = "草稿"
    case superseded = "已替代"
    case withdrawn = "已撤回"
}

enum ReviewInboxTimelineStage: String, CaseIterable {
    case confirmedAction = "确认动作"
    case operationLedger = "执行账本"
    case larkAdapter = "飞书适配器"
    case auditEvent = "审计事件"
}

enum ReviewInboxTimelineStatus: String {
    case pending = "待处理"
    case ok = "正常"
    case error = "异常"
}

struct ReviewInboxDisplayItem: Identifiable, Equatable {
    let id: String
    let proposedActionID: String
    let proposedActionVersion: UInt64
    let objectiveTitle: String
    let keyResultTitle: String
    let ownerName: String
    let weekLabel: String
    let riskLevel: ReviewInboxRiskLevel
    let riskReason: String
    let confidenceScore: Double
    var status: ReviewInboxDisplayStatus
    let ledgerStatus: ReviewInboxLedgerStatus?
    let operationID: String?
    let lastUpdatedAt: String
    let syncCursor: UInt64

    init(
        id: String,
        proposedActionID: String,
        proposedActionVersion: UInt64,
        objectiveTitle: String,
        keyResultTitle: String,
        ownerName: String,
        weekLabel: String,
        riskLevel: ReviewInboxRiskLevel,
        riskReason: String,
        confidenceScore: Double,
        status: ReviewInboxDisplayStatus,
        ledgerStatus: ReviewInboxLedgerStatus? = nil,
        operationID: String? = nil,
        lastUpdatedAt: String,
        syncCursor: UInt64
    ) {
        self.id = id
        self.proposedActionID = proposedActionID
        self.proposedActionVersion = proposedActionVersion
        self.objectiveTitle = objectiveTitle
        self.keyResultTitle = keyResultTitle
        self.ownerName = ownerName
        self.weekLabel = weekLabel
        self.riskLevel = riskLevel
        self.riskReason = riskReason
        self.confidenceScore = confidenceScore
        self.status = status
        self.ledgerStatus = ledgerStatus
        self.operationID = operationID
        self.lastUpdatedAt = lastUpdatedAt
        self.syncCursor = syncCursor
    }
}

struct ReviewInboxDisplayEvidence: Identifiable {
    let id: String
    let reviewItemId: String
    let sourceType: ReviewInboxEvidenceSource
    let sourceRef: String
    let capturedAt: String
    let summary: String
    let signalType: ReviewInboxSignal
    let trustScore: Double
}

struct ReviewInboxSuggestedAction: Identifiable, Equatable {
    let id: String
    let reviewItemId: String
    let version: UInt64
    let actionType: ReviewInboxActionType
    let rationale: String
    let expectedImpact: String
    let dryRunResultSummary: String
    let estimatedWriteTargetsCount: Int
    var gateState: ReviewInboxGateState

    var canEnterProductionExecution: Bool {
        actionType == .updateProgress && !dryRunResultSummary.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }
}

struct ReviewInboxTimelineEvent: Identifiable {
    let id: String
    let actionId: String
    let stage: ReviewInboxTimelineStage
    let stageStatus: ReviewInboxTimelineStatus
    let timestamp: String
    let message: String
    let idempotencyKey: String
}

enum ReviewInboxFilter: String, CaseIterable, Identifiable {
    case all = "全部"
    case highRisk = "高风险"
    case needsConfirmation = "待确认"
    case confirmed = "已确认"
    case executing = "执行中"
    case failed = "失败"
    case executed = "已执行"
    case cancelled = "已取消"
    case rejected = "已拒绝"

    var id: String { rawValue }

    func includes(_ item: ReviewInboxDisplayItem) -> Bool {
        switch self {
        case .all:
            return true
        case .highRisk:
            return item.riskLevel == .critical || item.riskLevel == .high
        case .needsConfirmation:
            return item.status == .needsConfirmation || item.status == .new
        case .confirmed:
            return item.status == .confirmed
        case .executing:
            return item.status == .executing
        case .failed:
            return item.status == .failed
        case .executed:
            return item.status == .executed
        case .cancelled:
            return item.status == .cancelled
        case .rejected:
            return item.status == .rejected
        }
    }
}
