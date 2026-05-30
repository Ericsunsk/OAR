extension ReviewInboxMockData {
    static let actions: [ReviewInboxSuggestedAction] = [
        ReviewInboxSuggestedAction(
            id: "act-001",
            reviewItemId: "review-001",
            version: 1,
            actionType: .updateProgress,
            rationale: "写入一条进展，说明 KR 过久未更新和安全评审阻塞。",
            expectedImpact: "周会前把风险同步到飞书 OKR。",
            dryRunResultSummary: "将更新 1 条 KR 进展，附 3 条证据引用；不修改 owner、target、权重。",
            estimatedWriteTargetsCount: 1,
            gateState: .pending
        ),
        ReviewInboxSuggestedAction(
            id: "act-002",
            reviewItemId: "review-001",
            version: 1,
            actionType: .scheduleReview,
            rationale: "为两个试点决策安排负责人同步。",
            expectedImpact: "把被动风险转成明确跟进。",
            dryRunResultSummary: "仅生成会议草稿；此原型不会真实创建会议。",
            estimatedWriteTargetsCount: 0,
            gateState: .pending
        ),
        ReviewInboxSuggestedAction(
            id: "act-003",
            reviewItemId: "review-002",
            version: 1,
            actionType: .pingOwner,
            rationale: "询问授权重试问题的最新缓解计划。",
            expectedImpact: "确认低进度信号是否仍然有效。",
            dryRunResultSummary: "仅生成提醒草稿；不会发送消息。",
            estimatedWriteTargetsCount: 0,
            gateState: .pending
        ),
        ReviewInboxSuggestedAction(
            id: "act-004",
            reviewItemId: "review-003",
            version: 1,
            actionType: .createTask,
            rationale: "补一个拒绝事件进入 outbox 的审计任务。",
            expectedImpact: "减少审计链路断点。",
            dryRunResultSummary: "将生成任务草稿，仅包含安全摘要。",
            estimatedWriteTargetsCount: 0,
            gateState: .approved
        ),
        ReviewInboxSuggestedAction(
            id: "act-005",
            reviewItemId: "review-004",
            version: 1,
            actionType: .updateProgress,
            rationale: "记录客户复盘节奏正常，并保留证据可信度说明。",
            expectedImpact: "不过度放大低风险事项。",
            dryRunResultSummary: "模拟已执行：1 条进展记录，无原始会议内容。",
            estimatedWriteTargetsCount: 1,
            gateState: .approved
        ),
        ReviewInboxSuggestedAction(
            id: "act-006",
            reviewItemId: "review-005",
            version: 1,
            actionType: .updateProgress,
            rationale: "写入执行延迟风险说明，等待适配器完成。",
            expectedImpact: "把排队中的执行状态暴露给 owner。",
            dryRunResultSummary: "执行中：账本已进入队列，尚无适配器成功回执。",
            estimatedWriteTargetsCount: 1,
            gateState: .approved
        ),
        ReviewInboxSuggestedAction(
            id: "act-007",
            reviewItemId: "review-006",
            version: 1,
            actionType: .updateProgress,
            rationale: "记录飞书适配器失败并等待人工复核。",
            expectedImpact: "避免把失败执行误认为已完成。",
            dryRunResultSummary: "失败：适配器返回可重试错误，未写入平台。",
            estimatedWriteTargetsCount: 1,
            gateState: .approved
        )
    ]
}
