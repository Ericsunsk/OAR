import XCTest
@testable import OAR

final class RemoteAuthProviderTests: XCTestCase {
    override func tearDown() {
        AuthProviderURLProtocol.handler = nil
        super.tearDown()
    }

    func testSignOutSendsOARSessionAuthorizationHeader() async throws {
        AuthProviderURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.path, "/auth/logout")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer oar_session_test")

            return (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!,
                Data(#"{"status":"signed_out"}"#.utf8)
            )
        }

        let provider = RemoteAuthProvider(
            baseURL: URL(string: "https://oar.example.test")!,
            urlSession: Self.urlSession
        )

        try await provider.signOut(appSession: Self.appSession)
    }

    func testSignOutMapsUnauthorizedSession() async throws {
        AuthProviderURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/auth/logout")

            return (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 401,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!,
                Data(#"{"error":"invalid_oar_session"}"#.utf8)
            )
        }

        let provider = RemoteAuthProvider(
            baseURL: URL(string: "https://oar.example.test")!,
            urlSession: Self.urlSession
        )

        do {
            try await provider.signOut(appSession: Self.appSession)
            XCTFail("expected invalid session")
        } catch AuthProviderError.invalidSession {
        } catch {
            XCTFail("expected invalid session, got \(error)")
        }
    }

    private static let appSession = AppSession(
        sessionID: "oar_session_test",
        user: AuthenticatedUser(id: "user_test", displayName: "测试用户", tenantName: "测试租户")
    )

    private static var urlSession: URLSession {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [AuthProviderURLProtocol.self]
        return URLSession(configuration: configuration)
    }
}

private final class AuthProviderURLProtocol: HTTPURLProtocolStub {}
