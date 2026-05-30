import XCTest
@testable import OAR

final class RemoteReviewInboxDataProviderTests: XCTestCase {
    override func tearDown() {
        TestURLProtocol.handler = nil
        super.tearDown()
    }

    func testLoadSnapshotSendsOARSessionAuthorizationHeader() async throws {
        TestURLProtocol.handler = { request in
            XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer oar_session_test")
            XCTAssertNil(request.value(forHTTPHeaderField: "X-Feishu-Access-Token"))
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")

            return (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!,
                Self.snapshotJSON
            )
        }

        let provider = Self.provider()

        let snapshot = try await provider.loadSnapshot()

        XCTAssertEqual(snapshot.items.first?.id, "ri_1")
        XCTAssertEqual(snapshot.ledgerEvents.map(\.id), ["le_remote_1"])
    }

    func testSubmitDecisionEncodesActionVersionAndSyncCursor() async throws {
        TestURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer oar_session_test")

            let body = try URLRequestBodyTestSupport.bodyData(from: request)
            let json = try JSONSerialization.jsonObject(with: body) as? [String: Any]
            XCTAssertEqual(json?["action_id"] as? String, "pa_1")
            XCTAssertEqual(json?["action_version"] as? Int, 2)
            XCTAssertEqual(json?["expected_sync_cursor"] as? Int, 42)

            return (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!,
                Self.snapshotJSON
            )
        }

        let provider = Self.provider()

        _ = try await provider.submitDecision(
            .approve(actionID: "pa_1", version: 2, expectedSyncCursor: 42, note: "确认"),
            snapshot: ReviewInboxDisplaySnapshot(items: [], evidence: [], actions: [], ledgerEvents: [])
        )
    }

    func testConflictMapsToStaleSyncCursor() async {
        await assertLoadSnapshot(statusCode: 409, mapsTo: .staleSyncCursor)
    }

    func testUnauthorizedStatusesMapToUnauthorized() async {
        await assertLoadSnapshot(statusCode: 401, mapsTo: .unauthorized)
        await assertLoadSnapshot(statusCode: 403, mapsTo: .unauthorized)
    }

    func testValidationErrorMapsToUnsupportedAction() async {
        await assertLoadSnapshot(statusCode: 422, mapsTo: .unsupportedAction)
    }

    func testValidationErrorSafeMessageMapsToRemoteRejected() async {
        let payload = Data(
            """
            {
              "reason": "validation_failed",
              "safe_message": "该动作已被其他审批覆盖，请刷新后重试。"
            }
            """.utf8
        )

        TestURLProtocol.handler = { request in
            (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 422,
                    httpVersion: nil,
                    headerFields: nil
                )!,
                payload
            )
        }

        let provider = Self.provider()

        do {
            _ = try await provider.loadSnapshot()
            XCTFail("Expected remoteRejected error")
        } catch let error as ReviewInboxDataProviderError {
            guard case let .remoteRejected(message) = error else {
                XCTFail("Unexpected error: \(error)")
                return
            }
            XCTAssertEqual(message, "该动作已被其他审批覆盖，请刷新后重试。")
            XCTAssertEqual(error.localizedDescription, "该动作已被其他审批覆盖，请刷新后重试。")
        } catch {
            XCTFail("Unexpected error: \(error)")
        }
    }

    func testServerErrorMapsToServerUnavailable() async {
        await assertLoadSnapshot(statusCode: 503, mapsTo: .serverUnavailable)
    }

    private func assertLoadSnapshot(
        statusCode: Int,
        responseData: Data = Data(),
        mapsTo expectedError: ReviewInboxDataProviderError,
        file: StaticString = #filePath,
        line: UInt = #line
    ) async {
        TestURLProtocol.handler = { request in
            (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: statusCode,
                    httpVersion: nil,
                    headerFields: nil
                )!,
                responseData
            )
        }

        let provider = Self.provider()

        do {
            _ = try await provider.loadSnapshot()
            XCTFail("Expected \(expectedError) error", file: file, line: line)
        } catch let error as ReviewInboxDataProviderError {
            XCTAssertEqual(error.localizedDescription, expectedError.localizedDescription, file: file, line: line)
        } catch {
            XCTFail("Unexpected error: \(error)", file: file, line: line)
        }
    }

    private static let appSession = AppSession(
        sessionID: "oar_session_test",
        user: AuthenticatedUser(id: "user_test", displayName: "测试用户", tenantName: "测试租户")
    )

    private static var urlSession: URLSession {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [TestURLProtocol.self]
        return URLSession(configuration: configuration)
    }

    private static func provider() -> RemoteReviewInboxDataProvider {
        RemoteReviewInboxDataProvider(
            baseURL: URL(string: "https://oar.example.test")!,
            appSession: appSession,
            urlSession: urlSession
        )
    }

    private static let snapshotJSON = Data(
        """
        {
          "contract_version": 1,
          "generated_at": "2026-05-28T10:00:00Z",
          "items": [
            {
              "id": "ri_1",
              "tenant_id": "t_1",
              "user_id": "u_1",
              "proposed_action_id": "pa_1",
              "proposed_action_version": 2,
              "objective_title": "提升复盘节奏",
              "key_result_title": "每周风险处理完成率 90%",
              "owner_display_name": "陈敏",
              "week_label": "2026 第 22 周",
              "risk_score": 92,
              "priority": 10,
              "risk_reason": "连续两周未处理高风险项。",
              "confidence_score": 0.88,
              "status": "open",
              "sync_cursor": 42,
              "updated_at_display": "5 月 28 日",
              "ledger_status": null,
              "operation_id": null
            }
          ],
          "proposed_actions": [],
          "evidence": [],
          "ledger_events": [
            {
              "id": "le_remote_1",
              "action_id": "pa_1",
              "stage": "operation_ledger",
              "stage_status": "ok",
              "timestamp_display": "2026-05-28T10:01:00Z",
              "message": "Operation ledger confirmed.",
              "idempotency_key": "decision:pa_1:v2:confirm"
            }
          ]
        }
        """.utf8
    )
}

private final class TestURLProtocol: HTTPURLProtocolStub {}
