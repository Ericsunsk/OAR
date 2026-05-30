extension ReviewInboxMockData {
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
