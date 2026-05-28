import XCTest
@testable import OARReviewInbox

@MainActor
final class AuthViewModelTests: XCTestCase {
    func testStartFeishuLoginCreatesWaitingSession() async {
        let sessionStore = AppSessionStore()
        let model = AuthViewModel(provider: MockAuthProvider(), sessionStore: sessionStore)

        await model.startFeishuLogin()

        XCTAssertNotNil(model.qrSession)
        XCTAssertEqual(model.statusText, "连接登录事件")
        XCTAssertFalse(sessionStore.isAuthenticated)
    }

    func testMissingBackendKeepsLoginSignedOut() async {
        let sessionStore = AppSessionStore()
        let model = AuthViewModel(provider: MissingBackendAuthProvider(), sessionStore: sessionStore)

        await model.startFeishuLogin()

        XCTAssertNil(model.qrSession)
        XCTAssertEqual(model.statusText, "等待开始")
        XCTAssertFalse(sessionStore.isAuthenticated)
        XCTAssertEqual(
            model.errorMessage,
            "创建飞书登录会话失败：\(AuthProviderError.missingBackendConfiguration.localizedDescription)"
        )
    }

    func testPollingMockSessionAuthorizesAppSession() async {
        let sessionStore = AppSessionStore()
        let model = AuthViewModel(provider: MockAuthProvider(), sessionStore: sessionStore)

        await model.startFeishuLogin()
        await model.pollOnce()

        XCTAssertFalse(sessionStore.isAuthenticated)

        await model.pollOnce()

        XCTAssertTrue(sessionStore.isAuthenticated)
        XCTAssertEqual(sessionStore.session?.user.displayName, "陈敏")
        XCTAssertEqual(model.statusText, "已登录")
    }

    func testSSEMockSessionAuthorizesAppSession() async {
        let sessionStore = AppSessionStore()
        let model = AuthViewModel(provider: MockAuthProvider(), sessionStore: sessionStore)

        await model.startFeishuLogin()

        try? await Task.sleep(nanoseconds: 20_000_000)

        XCTAssertTrue(sessionStore.isAuthenticated)
        XCTAssertEqual(model.statusText, "已登录")
    }

    func testStaleSSEEventDoesNotAuthorizeCurrentSession() async {
        let sessionStore = AppSessionStore()
        let provider = StaleEventAuthProvider()
        let model = AuthViewModel(provider: provider, sessionStore: sessionStore)

        await model.startFeishuLogin()

        try? await Task.sleep(nanoseconds: 20_000_000)

        XCTAssertFalse(sessionStore.isAuthenticated)
        XCTAssertEqual(model.statusText, "等待扫码")
    }

    func testCancelLoginReturnsToSignedOut() async {
        let sessionStore = AppSessionStore()
        let model = AuthViewModel(provider: MockAuthProvider(), sessionStore: sessionStore)

        await model.startFeishuLogin()
        model.cancelLogin()

        XCTAssertNil(model.qrSession)
        XCTAssertEqual(model.statusText, "等待开始")
        XCTAssertFalse(sessionStore.isAuthenticated)
    }
}

private final class StaleEventAuthProvider: AuthProviding {
    func createFeishuQRCodeSession() async throws -> FeishuQRCodeAuthSession {
        FeishuQRCodeAuthSession(
            id: "current-session",
            qrPageURL: URL(string: "https://open.feishu.cn/current")!,
            expiresAt: Date().addingTimeInterval(300)
        )
    }

    func pollFeishuQRCodeSession(_ sessionID: String) async throws -> AuthSessionState {
        .waitingForScan(
            FeishuQRCodeAuthSession(
                id: sessionID,
                qrPageURL: URL(string: "https://open.feishu.cn/current")!,
                expiresAt: Date().addingTimeInterval(300)
            )
        )
    }

    func subscribeFeishuQRCodeSession(_ sessionID: String) -> AsyncThrowingStream<AuthLoginEvent, Error> {
        AsyncThrowingStream { continuation in
            continuation.yield(
                .authorized(
                    sessionID: "old-session",
                    appSession: AppSession(
                        sessionID: "stale-oar-session",
                        user: AuthenticatedUser(
                            id: "user_stale",
                            displayName: "过期用户",
                            tenantName: "旧租户"
                        )
                    )
                )
            )
            continuation.finish()
        }
    }

    func signOut() async throws {
    }
}
