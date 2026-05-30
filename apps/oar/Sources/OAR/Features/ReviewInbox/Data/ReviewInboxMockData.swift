import Foundation

enum ReviewInboxMockData {
    static let reviewItems: [ReviewInboxDisplayItem] = [
        ReviewInboxDisplayItem(
            id: "review-001",
            proposedActionID: "act-001",
            proposedActionVersion: 1,
            objectiveTitle: "扩大企业试点采用",
            keyResultTitle: "激活 12 个合格试点团队",
            ownerName: "陈敏",
            weekLabel: "2026 第 22 周",
            riskLevel: .critical,
            riskReason: "19 天未更新进展，两个上线任务仍被阻塞。",
            confidenceScore: 0.91,
            status: .needsConfirmation,
            lastUpdatedAt: "5 月 9 日",
            syncCursor: 101
        ),
        ReviewInboxDisplayItem(
            id: "review-002",
            proposedActionID: "act-003",
            proposedActionVersion: 1,
            objectiveTitle: "提升新用户接入稳定性",
            keyResultTitle: "首轮配置失败率降至 4% 以下",
            ownerName: "周然",
            weekLabel: "2026 第 22 周",
            riskLevel: .high,
            riskReason: "当前进度低于节奏 18 个点，事故记录指向授权重试问题。",
            confidenceScore: 0.84,
            status: .new,
            lastUpdatedAt: "5 月 18 日",
            syncCursor: 102
        ),
        ReviewInboxDisplayItem(
            id: "review-003",
            proposedActionID: "act-004",
            proposedActionVersion: 1,
            objectiveTitle: "稳定复盘执行系统",
            keyResultTitle: "动作审计覆盖率达到 95%",
            ownerName: "赵一",
            weekLabel: "2026 第 22 周",
            riskLevel: .medium,
            riskReason: "审计 outbox 仍有重试堆积，本周没有外部投递健康记录。",
            confidenceScore: 0.73,
            status: .confirmed,
            lastUpdatedAt: "5 月 23 日",
            syncCursor: 103
        ),
        ReviewInboxDisplayItem(
            id: "review-004",
            proposedActionID: "act-005",
            proposedActionVersion: 1,
            objectiveTitle: "建立客户复盘节奏",
            keyResultTitle: "完成 8 场设计伙伴复盘",
            ownerName: "林浩",
            weekLabel: "2026 第 22 周",
            riskLevel: .low,
            riskReason: "日历节奏正常，但两条证据可信度偏低。",
            confidenceScore: 0.58,
            status: .executed,
            lastUpdatedAt: "5 月 26 日",
            syncCursor: 104
        ),
        ReviewInboxDisplayItem(
            id: "review-005",
            proposedActionID: "act-006",
            proposedActionVersion: 1,
            objectiveTitle: "提升代理执行可靠性",
            keyResultTitle: "执行队列延迟控制在 2 分钟内",
            ownerName: "许诺",
            weekLabel: "2026 第 22 周",
            riskLevel: .medium,
            riskReason: "后台已确认动作，执行账本仍在等待适配器回执。",
            confidenceScore: 0.69,
            status: .executing,
            lastUpdatedAt: "5 月 27 日",
            syncCursor: 105
        ),
        ReviewInboxDisplayItem(
            id: "review-006",
            proposedActionID: "act-007",
            proposedActionVersion: 1,
            objectiveTitle: "保持平台写入健康",
            keyResultTitle: "飞书写入失败重试不超过 1 次",
            ownerName: "王启",
            weekLabel: "2026 第 22 周",
            riskLevel: .low,
            riskReason: "上一条已确认动作在适配器阶段失败，需要人工复核后重试。",
            confidenceScore: 0.62,
            status: .failed,
            lastUpdatedAt: "5 月 27 日",
            syncCursor: 106
        )
    ]

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

    static let ledgerEvents: [ReviewInboxTimelineEvent] = [
        ReviewInboxTimelineEvent(
            id: "led-001",
            actionId: "act-005",
            stage: .confirmedAction,
            stageStatus: .ok,
            timestamp: "5 月 27 日 08:33",
            message: "林浩已确认。",
            idempotencyKey: "tenant:t_demo:pa:review-004:v1:confirm"
        ),
        ReviewInboxTimelineEvent(
            id: "led-002",
            actionId: "act-005",
            stage: .operationLedger,
            stageStatus: .ok,
            timestamp: "5 月 27 日 08:33",
            message: "账本进入执行中。",
            idempotencyKey: "tenant:t_demo:pa:review-004:v1:confirm"
        ),
        ReviewInboxTimelineEvent(
            id: "led-003",
            actionId: "act-005",
            stage: .larkAdapter,
            stageStatus: .ok,
            timestamp: "5 月 27 日 08:34",
            message: "模拟适配器完成。",
            idempotencyKey: "tenant:t_demo:pa:review-004:v1:confirm"
        ),
        ReviewInboxTimelineEvent(
            id: "led-004",
            actionId: "act-005",
            stage: .auditEvent,
            stageStatus: .ok,
            timestamp: "5 月 27 日 08:34",
            message: "审计事件已记录。",
            idempotencyKey: "tenant:t_demo:pa:review-004:v1:confirm"
        ),
        ReviewInboxTimelineEvent(
            id: "led-005",
            actionId: "act-004",
            stage: .confirmedAction,
            stageStatus: .ok,
            timestamp: "5 月 27 日 09:12",
            message: "赵一已确认，等待后台执行。",
            idempotencyKey: "tenant:t_demo:pa:review-003:v1:confirm"
        ),
        ReviewInboxTimelineEvent(
            id: "led-006",
            actionId: "act-006",
            stage: .confirmedAction,
            stageStatus: .ok,
            timestamp: "5 月 27 日 09:20",
            message: "许诺已确认。",
            idempotencyKey: "tenant:t_demo:pa:review-005:v1:confirm"
        ),
        ReviewInboxTimelineEvent(
            id: "led-007",
            actionId: "act-006",
            stage: .operationLedger,
            stageStatus: .pending,
            timestamp: "5 月 27 日 09:20",
            message: "执行账本已入队，等待适配器回执。",
            idempotencyKey: "tenant:t_demo:pa:review-005:v1:confirm"
        ),
        ReviewInboxTimelineEvent(
            id: "led-008",
            actionId: "act-007",
            stage: .confirmedAction,
            stageStatus: .ok,
            timestamp: "5 月 27 日 09:24",
            message: "王启已确认。",
            idempotencyKey: "tenant:t_demo:pa:review-006:v1:confirm"
        ),
        ReviewInboxTimelineEvent(
            id: "led-009",
            actionId: "act-007",
            stage: .operationLedger,
            stageStatus: .ok,
            timestamp: "5 月 27 日 09:24",
            message: "执行账本开始处理。",
            idempotencyKey: "tenant:t_demo:pa:review-006:v1:confirm"
        ),
        ReviewInboxTimelineEvent(
            id: "led-010",
            actionId: "act-007",
            stage: .larkAdapter,
            stageStatus: .error,
            timestamp: "5 月 27 日 09:25",
            message: "适配器返回可重试错误，平台未写入。",
            idempotencyKey: "tenant:t_demo:pa:review-006:v1:confirm"
        ),
        ReviewInboxTimelineEvent(
            id: "led-011",
            actionId: "act-007",
            stage: .auditEvent,
            stageStatus: .ok,
            timestamp: "5 月 27 日 09:25",
            message: "失败审计事件已记录。",
            idempotencyKey: "tenant:t_demo:pa:review-006:v1:confirm"
        )
    ]
}
