import XCTest
@testable import OAR

final class AuthAPIContractTests: XCTestCase {
    func testCreateQRCodeSessionDecodesSnakeCaseResponse() throws {
        let json = """
        {
          "session_id": "qr_123",
          "qr_page_url": "https://oar.example.test/auth/feishu/qr/qr_123",
          "expires_at": "2026-05-28T06:30:00Z"
        }
        """

        let dto = try JSONDecoder().decode(
            CreateFeishuQRCodeSessionResponseDTO.self,
            from: Data(json.utf8)
        )
        let domain = try dto.toDomain()

        XCTAssertEqual(domain.id, "qr_123")
        XCTAssertEqual(domain.qrPageURL.absoluteString, "https://oar.example.test/auth/feishu/qr/qr_123")
    }

    func testAuthorizedStatusMapsToAppSessionWithoutFeishuTokens() throws {
        let json = """
        {
          "status": "authorized",
          "qr_session": null,
          "oar_session": {
            "session_id": "oar_session_123"
          },
          "user": {
            "id": "user_1",
            "display_name": "陈敏",
            "tenant_name": "OAR 测试租户"
          },
          "safe_message": null
        }
        """

        let dto = try JSONDecoder().decode(
            FeishuQRCodeSessionStatusResponseDTO.self,
            from: Data(json.utf8)
        )
        let state = try dto.toDomainState()

        guard case let .authorized(session) = state else {
            XCTFail("Expected authorized state")
            return
        }

        XCTAssertEqual(session.sessionID, "oar_session_123")
        XCTAssertEqual(session.user.displayName, "陈敏")
    }

    func testPendingStatusRequiresQRCodeSession() throws {
        let json = """
        {
          "status": "pending",
          "qr_session": null,
          "oar_session": null,
          "user": null,
          "safe_message": null
        }
        """

        let dto = try JSONDecoder().decode(
            FeishuQRCodeSessionStatusResponseDTO.self,
            from: Data(json.utf8)
        )

        XCTAssertThrowsError(try dto.toDomainState())
    }

    func testSSEAuthorizedEventMapsToLoginEvent() throws {
        let json = """
        {
          "event": "authorized",
          "session_id": "qr_123",
          "qr_session": null,
          "oar_session": {
            "session_id": "oar_session_123"
          },
          "user": {
            "id": "user_1",
            "display_name": "陈敏",
            "tenant_name": "OAR 测试租户"
          },
          "safe_message": null,
          "event_id": "evt_1"
        }
        """

        let dto = try JSONDecoder().decode(AuthLoginEventDTO.self, from: Data(json.utf8))
        let event = try dto.toDomainEvent()

        guard case let .authorized(sessionID, session) = event else {
            XCTFail("Expected authorized event")
            return
        }

        XCTAssertEqual(sessionID, "qr_123")
        XCTAssertEqual(session.sessionID, "oar_session_123")
    }
}
