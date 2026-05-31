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

            let body = try URLRequestBodyTestSupport.bodyData(from: request)
            let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
            let messages = try XCTUnwrap(json["messages"] as? [[String: Any]])
            XCTAssertEqual(messages.count, 13)
            XCTAssertEqual(messages.last?["role"] as? String, "user")
            XCTAssertEqual(messages.last?["text"] as? String, "解释风险")
            let context = try XCTUnwrap(json["context"] as? [String: Any])
            XCTAssertEqual(
                context["evidence_summaries"] as? [String],
                ["连续两周延期", "会议纪要显示两个试点需要周五前决策"]
            )
            let evidenceRefs = try XCTUnwrap(context["evidence_refs"] as? [[String: Any]])
            XCTAssertEqual(evidenceRefs.count, 2)
            XCTAssertEqual(evidenceRefs[0]["source_type"] as? String, "okr")
            XCTAssertEqual(evidenceRefs[0]["source_ref"] as? String, "okr://cycle/2026q2/objective/ent-growth")
            XCTAssertEqual(evidenceRefs[0]["summary"] as? String, "连续两周延期")
            XCTAssertEqual(evidenceRefs[1]["source_type"] as? String, "meeting")
            XCTAssertEqual(evidenceRefs[1]["source_ref"] as? String, "minutes://enterprise-weekly-sync")
            XCTAssertEqual(evidenceRefs[1]["summary"] as? String, "会议纪要显示两个试点需要周五前决策")
            XCTAssertEqual(context["workspace_summary"] as? String, "工作区摘要：共 2 个风险，严重/高 1 个。")
            XCTAssertEqual(context["workspace_signals"] as? [String], ["严重｜KR 风险｜owner：陈敏｜置信 91%"])
            XCTAssertEqual(
                context["pending_action_summaries"] as? [String],
                ["KR 风险｜更新进展｜gate：待处理｜dry-run：将更新 1 条 KR 进展。"]
            )
            XCTAssertEqual(
                context["ledger_event_summaries"] as? [String],
                ["审计事件｜正常｜2026-05-30T10:02:00Z｜ActionID act_1｜AuditEvent 已记录"]
            )
            XCTAssertFalse(String(data: body, encoding: .utf8)?.contains("sk-") ?? true)

            return (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "text/event-stream"]
                )!,
                Self.sse(
                    """
                    : keep-alive

                    data: {"event":"delta","delta":"风险"}

                    data: {"event":"delta","delta":"来自延期。"}

                    data: {"event":"completed"}

                    """
                )
            )
        }

        let provider = Self.provider()
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
                            sourceType: "okr",
                            sourceRef: "okr://cycle/2026q2/objective/ent-growth",
                            summary: "连续两周延期"
                        ),
                        AgentEvidenceRef(
                            sourceType: "meeting",
                            sourceRef: "minutes://enterprise-weekly-sync",
                            summary: "会议纪要显示两个试点需要周五前决策"
                        )
                    ],
                    workspaceSummary: "工作区摘要：共 2 个风险，严重/高 1 个。",
                    workspaceSignals: ["严重｜KR 风险｜owner：陈敏｜置信 91%"],
                    pendingActionSummaries: ["KR 风险｜更新进展｜gate：待处理｜dry-run：将更新 1 条 KR 进展。"],
                    ledgerEventSummaries: ["审计事件｜正常｜2026-05-30T10:02:00Z｜ActionID act_1｜AuditEvent 已记录"]
                )
            )
        )

        XCTAssertEqual(events, [.delta("风险"), .delta("来自延期。"), .completed])
    }

    func testHTTPStatusErrorsMapToProviderErrors() async {
        await Self.assertStreamError(statusCode: 401, mapsTo: .unauthorized)
        await Self.assertStreamError(statusCode: 403, mapsTo: .unauthorized)
        await Self.assertStreamError(statusCode: 404, mapsTo: .serverUnavailable)
        await Self.assertStreamError(statusCode: 406, mapsTo: .serverUnavailable)
        await Self.assertStreamError(statusCode: 422, mapsTo: .serverUnavailable)
        await Self.assertStreamError(statusCode: 429, mapsTo: .serverUnavailable)
        await Self.assertStreamError(statusCode: 500, mapsTo: .serverUnavailable)
        await Self.assertStreamError(statusCode: 418, mapsTo: .invalidResponse)
    }

    func testUnknownStreamEventIsIgnoredForForwardCompatibility() async throws {
        AgentTestURLProtocol.handler = { request in
            (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "text/event-stream"]
                )!,
                Self.sse(
                    """
                    data: {"event":"delta","delta":"风险"}

                    data: {"event":"metadata","message":"future event"}

                    data: {"event":"completed"}

                    """
                )
            )
        }

        let events = try await Self.collectEvents(
            from: Self.provider().stream(
                messages: [AgentMessage(role: .user, text: "hi")],
                context: .empty
            )
        )

        XCTAssertEqual(events, [.delta("风险"), .completed])
    }

    func testStreamErrorEventsMapToServerUnavailable() async {
        for code in ["invalid_upstream_event", "upstream_unavailable", "upstream_error"] {
            await Self.assertStreamBody(
                """
                data: {"event":"error","error":"\(code)"}

                """,
                mapsTo: .serverUnavailable
            )
        }

        await Self.assertStreamBody(
            """
            data: {"event":"error","code":"upstream_error"}

            """,
            mapsTo: .serverUnavailable
        )
    }

    func testStreamErrorCodeIsNotExposedToUI() async {
        await Self.assertStreamBody(
            """
            data: {"event":"error","error":"oar_session_secret"}

            """,
            mapsTo: .serverUnavailable
        )
    }

    func testBlankDeltaIsIgnoredAndDoesNotCountAsContent() async {
        await Self.assertStreamBody(
            """
            data: {"event":"delta","delta":"   "}

            data: {"event":"completed"}

            """,
            mapsTo: .invalidResponse
        )
    }

    func testCompletedStreamWithoutDeltaMapsToInvalidResponse() async {
        await Self.assertStreamBody(
            """
            data: {"event":"completed"}

            """,
            mapsTo: .invalidResponse
        )
    }

    func testMalformedStreamEventMapsToInvalidResponse() async {
        await Self.assertStreamBody(
            """
            data: not-json

            """,
            mapsTo: .invalidResponse
        )
    }

    func testEOFBeforeCompletedMapsToInvalidResponse() async {
        await Self.assertStreamBody(
            """
            data: {"event":"delta","delta":"partial"}

            """,
            mapsTo: .invalidResponse
        )
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

    private static func provider() -> RemoteAgentProvider {
        RemoteAgentProvider(
            baseURL: URL(string: "https://oar.example.test")!,
            appSession: Self.appSession,
            urlSession: Self.urlSession
        )
    }

    private static func assertStreamError(
        statusCode: Int,
        mapsTo expectedError: AgentProviderError,
        file: StaticString = #filePath,
        line: UInt = #line
    ) async {
        await Self.assertStreamFailure(
            response: { request in
                (
                    HTTPURLResponse(
                        url: request.url!,
                        statusCode: statusCode,
                        httpVersion: nil,
                        headerFields: nil
                    )!,
                    Data("error".utf8)
                )
            },
            mapsTo: expectedError,
            file: file,
            line: line
        )
    }

    private static func assertStreamBody(
        _ text: String,
        mapsTo expectedError: AgentProviderError,
        file: StaticString = #filePath,
        line: UInt = #line
    ) async {
        await Self.assertStreamFailure(
            response: { request in
                (
                    HTTPURLResponse(
                        url: request.url!,
                        statusCode: 200,
                        httpVersion: nil,
                        headerFields: ["Content-Type": "text/event-stream"]
                    )!,
                    Self.sse(text)
                )
            },
            mapsTo: expectedError,
            file: file,
            line: line
        )
    }

    private static func assertStreamFailure(
        response: @escaping (URLRequest) throws -> (HTTPURLResponse, Data),
        mapsTo expectedError: AgentProviderError,
        file: StaticString = #filePath,
        line: UInt = #line
    ) async {
        AgentTestURLProtocol.handler = response

        do {
            try await Self.drain(
                Self.provider().stream(
                    messages: [AgentMessage(role: .user, text: "hi")],
                    context: .empty
                )
            )
            XCTFail("Expected \(expectedError)", file: file, line: line)
        } catch let error as AgentProviderError {
            XCTAssertEqual(error, expectedError, file: file, line: line)
            XCTAssertFalse(
                error.localizedDescription.contains("oar_session_secret"),
                file: file,
                line: line
            )
        } catch {
            XCTFail("Unexpected error: \(error)", file: file, line: line)
        }
    }

    private static func sse(_ text: String) -> Data {
        Data(text.utf8)
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

}

private final class AgentTestURLProtocol: HTTPURLProtocolStub {}
