@testable import OAR

func makeDisplayItem(
    id: String,
    status: ReviewInboxDisplayStatus = .needsConfirmation,
    proposedActionID: String = "act-default",
    proposedActionVersion: UInt64 = 1
) -> ReviewInboxDisplayItem {
    ReviewInboxDisplayItem(
        id: id,
        proposedActionID: proposedActionID,
        proposedActionVersion: proposedActionVersion,
        objectiveTitle: "目标",
        keyResultTitle: "关键结果",
        ownerName: "负责人",
        weekLabel: "2026 第 22 周",
        riskLevel: .high,
        riskReason: "需要复核",
        confidenceScore: 0.88,
        status: status,
        lastUpdatedAt: "2026-05-30T10:00:00Z",
        syncCursor: 1
    )
}

func makeSuggestedAction(
    id: String,
    reviewItemId: String,
    gateState: ReviewInboxGateState = .pending,
    version: UInt64 = 1
) -> ReviewInboxSuggestedAction {
    ReviewInboxSuggestedAction(
        id: id,
        reviewItemId: reviewItemId,
        version: version,
        actionType: .updateProgress,
        rationale: "补充进展",
        expectedImpact: "更新一条进展",
        dryRunResultSummary: "将更新 1 条进展记录。",
        estimatedWriteTargetsCount: 1,
        gateState: gateState
    )
}

func makeTimelineEvent(
    id: String,
    actionId: String,
    stage: ReviewInboxTimelineStage,
    status: ReviewInboxTimelineStatus,
    timestamp: String,
    message: String,
    idempotencyKey: String
) -> ReviewInboxTimelineEvent {
    ReviewInboxTimelineEvent(
        id: id,
        actionId: actionId,
        stage: stage,
        stageStatus: status,
        timestamp: timestamp,
        message: message,
        idempotencyKey: idempotencyKey
    )
}
