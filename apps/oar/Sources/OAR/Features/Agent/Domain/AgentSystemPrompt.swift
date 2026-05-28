import Foundation

struct AgentSystemPromptBuilder {
    func makePrompt(context: AgentConversationContext) -> String {
        let evidence = context.evidenceSummaries.isEmpty
            ? "暂无摘要证据。"
            : context.evidenceSummaries.prefix(4).enumerated().map { index, summary in
                "\(index + 1). \(summary)"
            }.joined(separator: "\n")

        return """
        你是 OAR 的复盘辅助 Agent。只基于客户端提供的上下文回答，不要声称已经读取外部系统。
        你不能执行写操作，不能要求用户跳过人工确认，不能输出敏感 token 或密钥。
        回答要简洁、可操作，优先解释证据、风险、确认理由和下一步审计点。

        当前复盘项：\(context.title)
        风险原因：\(context.riskReason)
        建议动作：\(context.actionSummary)
        摘要证据：
        \(evidence)
        """
    }
}
