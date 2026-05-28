import XCTest
@testable import OAR

final class AgentSystemPromptBuilderTests: XCTestCase {
    func testPromptIncludesSafetyBoundaryAndContext() {
        let prompt = AgentSystemPromptBuilder().makePrompt(
            context: AgentConversationContext(
                title: "KR 风险",
                riskReason: "连续延期",
                actionSummary: "更新进度 dry-run",
                evidenceSummaries: ["连续两周延期"]
            )
        )

        XCTAssertTrue(prompt.contains("只基于客户端提供的上下文回答"))
        XCTAssertTrue(prompt.contains("不要声称已经读取外部系统"))
        XCTAssertTrue(prompt.contains("不能执行写操作"))
        XCTAssertTrue(prompt.contains("不能要求用户跳过人工确认"))
        XCTAssertTrue(prompt.contains("不能输出敏感 token 或密钥"))
        XCTAssertTrue(prompt.contains("当前复盘项：KR 风险"))
        XCTAssertTrue(prompt.contains("风险原因：连续延期"))
        XCTAssertTrue(prompt.contains("建议动作：更新进度 dry-run"))
        XCTAssertTrue(prompt.contains("1. 连续两周延期"))
    }

    func testPromptUsesFallbackWhenEvidenceIsEmpty() {
        let prompt = AgentSystemPromptBuilder().makePrompt(context: .empty)

        XCTAssertTrue(prompt.contains("摘要证据：\n暂无摘要证据。"))
    }

    func testPromptLimitsEvidenceToFourSummaries() {
        let prompt = AgentSystemPromptBuilder().makePrompt(
            context: AgentConversationContext(
                title: "KR 风险",
                riskReason: "连续延期",
                actionSummary: "更新进度",
                evidenceSummaries: ["证据 1", "证据 2", "证据 3", "证据 4", "证据 5"]
            )
        )

        XCTAssertTrue(prompt.contains("1. 证据 1"))
        XCTAssertTrue(prompt.contains("4. 证据 4"))
        XCTAssertFalse(prompt.contains("证据 5"))
    }
}
