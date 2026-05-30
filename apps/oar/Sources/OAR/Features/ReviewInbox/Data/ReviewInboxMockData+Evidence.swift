extension ReviewInboxMockData {
    static let evidence: [ReviewInboxDisplayEvidence] = [
        ReviewInboxDisplayEvidence(
            id: "ev-001",
            reviewItemId: "review-001",
            sourceType: .okr,
            sourceRef: "okr://cycle/2026q2/objective/ent-growth",
            capturedAt: "5 月 27 日 09:14",
            summary: "上次进展停留在 19 天前，当前仍为 5/12 个试点。",
            signalType: .cadence,
            trustScore: 0.94
        ),
        ReviewInboxDisplayEvidence(
            id: "ev-002",
            reviewItemId: "review-001",
            sourceType: .task,
            sourceRef: "task://pilot-security-review",
            capturedAt: "5 月 26 日 17:30",
            summary: "安全问卷任务卡在应用权限说明。",
            signalType: .blocker,
            trustScore: 0.88
        ),
        ReviewInboxDisplayEvidence(
            id: "ev-003",
            reviewItemId: "review-001",
            sourceType: .meeting,
            sourceRef: "minutes://enterprise-weekly-sync",
            capturedAt: "5 月 24 日 15:00",
            summary: "会议纪要显示两个试点需要周五前决策。",
            signalType: .dependency,
            trustScore: 0.82
        ),
        ReviewInboxDisplayEvidence(
            id: "ev-004",
            reviewItemId: "review-002",
            sourceType: .doc,
            sourceRef: "doc://onboarding-runbook",
            capturedAt: "5 月 25 日 11:45",
            summary: "接入手册标注授权重试循环尚未解决。",
            signalType: .blocker,
            trustScore: 0.80
        ),
        ReviewInboxDisplayEvidence(
            id: "ev-005",
            reviewItemId: "review-003",
            sourceType: .okr,
            sourceRef: "okr://cycle/2026q2/objective/audit",
            capturedAt: "5 月 27 日 10:02",
            summary: "审计覆盖率提升到 87%，但拒绝事件尚未进入 outbox。",
            signalType: .progress,
            trustScore: 0.76
        ),
        ReviewInboxDisplayEvidence(
            id: "ev-006",
            reviewItemId: "review-004",
            sourceType: .calendar,
            sourceRef: "calendar://customer-cadence",
            capturedAt: "5 月 27 日 08:10",
            summary: "已预约 6 场客户复盘，2 场缺少跟进记录。",
            signalType: .progress,
            trustScore: 0.67
        )
    ]
}
