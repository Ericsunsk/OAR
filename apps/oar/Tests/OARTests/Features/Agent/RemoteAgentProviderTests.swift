import XCTest
@testable import OAR

final class RemoteAgentProviderTests: XCTestCase {
    override func tearDown() {
        AgentTestURLProtocol.handler = nil
        super.tearDown()
    }

    func testStreamUsesOARBackendRequestShapeAndYieldsDeltas() async throws {
        AgentTestURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.absoluteString, "https://oar.example.test/agent/stream")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer oar_session_secret")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "text/event-stream")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")

            let body = try Self.bodyData(from: request)
            let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
            let messages = try XCTUnwrap(json["messages"] as? [[String: Any]])
            XCTAssertEqual(messages.count, 13)
            XCTAssertEqual(messages.last?["role"] as? String, "user")
            XCTAssertEqual(messages.last?["text"] as? String, "解释风险")
            let context = try XCTUnwrap(json["context"] as? [String: Any])
            let evidenceRefs = try XCTUnwrap(context["evidence_refs"] as? [[String: Any]])
            XCTAssertEqual(evidenceRefs.count, 2)
            XCTAssertEqual(evidenceRefs[0]["source_type"] as? String, "OKR")
            XCTAssertEqual(evidenceRefs[0]["source_ref"] as? String, "okr://cycle/2026q2/objective/ent-growth")
            XCTAssertEqual(evidenceRefs[0]["summary"] as? String, "连续两周延期")
            XCTAssertEqual(evidenceRefs[1]["source_type"] as? String, "会议")
            XCTAssertEqual(evidenceRefs[1]["source_ref"] as? String, "minutes://enterprise-weekly-sync")
            XCTAssertEqual(evidenceRefs[1]["summary"] as? String, "会议纪要显示两个试点需要周五前决策")
            XCTAssertEqual(context["workspace_summary"] as? String, "工作区摘要：共 2 个风险，严重/高 1 个。")
            XCTAssertEqual(context["workspace_signals"] as? [String], ["严重｜KR 风险｜owner：陈敏｜置信 91%"])
            XCTAssertEqual(
                context["pending_action_summaries"] as? [String],
                ["KR 风险｜更新进展｜gate：待处理｜dry-run：将更新 1 条 KR 进展。"]
            )
            XCTAssertFalse(String(data: body, encoding: .utf8)?.contains("sk-") ?? true)

            return (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "text/event-stream"]
                )!,
                Data(
                    """
                    : keep-alive

                    data: {"event":"delta","delta":"风险"}

                    data: {"event":"delta","delta":"来自延期。"}

                    data: {"event":"completed"}

                    """.utf8
                )
            )
        }

        let provider = RemoteAgentProvider(
            baseURL: URL(string: "https://oar.example.test")!,
            appSession: Self.appSession,
            urlSession: Self.urlSession
        )
        let conversation = (0..<12).map { index in
            AgentMessage(role: .assistant, text: "历史回复 \(index)")
        } + [AgentMessage(role: .user, text: "解释风险")]
        let events = try await Self.collectEvents(
            from: provider.stream(
                messages: conversation,
                context: AgentConversationContext(
                    title: "KR 风险",
                    riskReason: "连续延期",
                    actionSummary: "更新进度",
                    evidenceSummaries: ["连续两周延期"],
                    evidenceRefs: [
                        AgentEvidenceRef(
                            sourceType: "OKR",
                            sourceRef: "okr://cycle/2026q2/objective/ent-growth",
                            summary: "连续两周延期"
                        ),
                        AgentEvidenceRef(
                            sourceType: "会议",
                            sourceRef: "minutes://enterprise-weekly-sync",
                            summary: "会议纪要显示两个试点需要周五前决策"
                        )
                    ],
                    workspaceSummary: "工作区摘要：共 2 个风险，严重/高 1 个。",
                    workspaceSignals: ["严重｜KR 风险｜owner：陈敏｜置信 91%"],
                    pendingActionSummaries: ["KR 风险｜更新进展｜gate：待处理｜dry-run：将更新 1 条 KR 进展。"]
                )
            )
        )

        XCTAssertEqual(events, [.delta("风险"), .delta("来自延期。"), .completed])
    }

    func testUnauthorizedStatusMapsToSessionError() async {
        AgentTestURLProtocol.handler = { request in
            (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 401,
                    httpVersion: nil,
                    headerFields: nil
                )!,
                Data("unauthorized".utf8)
            )
        }

        let provider = RemoteAgentProvider(
            baseURL: URL(string: "https://oar.example.test")!,
            appSession: Self.appSession,
            urlSession: Self.urlSession
        )

        do {
            try await Self.drain(provider.stream(messages: [AgentMessage(role: .user, text: "hi")], context: .empty))
            XCTFail("Expected unauthorized error")
        } catch let error as AgentProviderError {
            XCTAssertEqual(error, .unauthorized)
            XCTAssertFalse(error.localizedDescription.contains("oar_session_secret"))
        } catch {
            XCTFail("Unexpected error: \(error)")
        }
    }

    private static let appSession = AppSession(
        sessionID: "oar_session_secret",
        user: AuthenticatedUser(
            id: "user_1",
            displayName: "陈敏",
            tenantName: "OAR 测试租户"
        )
    )

    private static var urlSession: URLSession {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [AgentTestURLProtocol.self]
        return URLSession(configuration: configuration)
    }

    private static func collectEvents(
        from stream: AsyncThrowingStream<AgentStreamEvent, Error>
    ) async throws -> [AgentStreamEvent] {
        var events: [AgentStreamEvent] = []
        for try await event in stream {
            events.append(event)
        }
        return events
    }

    private static func drain(_ stream: AsyncThrowingStream<AgentStreamEvent, Error>) async throws {
        for try await _ in stream {
        }
    }

    private static func bodyData(from request: URLRequest) throws -> Data {
        if let httpBody = request.httpBody {
            return httpBody
        }

        let stream = try XCTUnwrap(request.httpBodyStream)
        stream.open()
        defer { stream.close() }

        var data = Data()
        let bufferSize = 1_024
        let buffer = UnsafeMutablePointer<UInt8>.allocate(capacity: bufferSize)
        defer { buffer.deallocate() }

        while stream.hasBytesAvailable {
            let bytesRead = stream.read(buffer, maxLength: bufferSize)
            if bytesRead > 0 {
                data.append(buffer, count: bytesRead)
            } else if bytesRead < 0 {
                throw stream.streamError ?? AgentProviderError.invalidResponse
            } else {
                break
            }
        }
        return data
    }
}

private final class AgentTestURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data))?

    override class func canInit(with request: URLRequest) -> Bool {
        true
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        do {
            let handler = try XCTUnwrap(Self.handler)
            let (response, data) = try handler(request)
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: data)
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {
    }
}
