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
    case executed = "已执行"
    case failed = "失败"
    case rejected = "已拒绝"

    var id: String { rawValue }
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
    let objectiveTitle: String
    let keyResultTitle: String
    let ownerName: String
    let weekLabel: String
    let riskLevel: ReviewInboxRiskLevel
    let riskReason: String
    let confidenceScore: Double
    var status: ReviewInboxDisplayStatus
    let lastUpdatedAt: String
    let syncCursor: UInt64
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
    case executed = "已执行"

    var id: String { rawValue }
}
