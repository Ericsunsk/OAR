import XCTest
@testable import OARReviewInbox

@MainActor
final class AuthViewModelTests: XCTestCase {
    func testStartFeishuLoginCreatesWaitingSession() async {
        let sessionStore = AppSessionStore()
        let model = AuthViewModel(sessionStore: sessionStore)

        await model.startFeishuLogin()

        XCTAssertNotNil(model.qrSession)
        XCTAssertEqual(model.statusText, "等待扫码")
        XCTAssertFalse(sessionStore.isAuthenticated)
    }

    func testPollingMockSessionAuthorizesAppSession() async {
        let sessionStore = AppSessionStore()
        let model = AuthViewModel(sessionStore: sessionStore)

        await model.startFeishuLogin()
        await model.pollOnce()

        XCTAssertFalse(sessionStore.isAuthenticated)

        await model.pollOnce()

        XCTAssertTrue(sessionStore.isAuthenticated)
        XCTAssertEqual(sessionStore.session?.user.displayName, "陈敏")
        XCTAssertEqual(model.statusText, "已登录")
    }

    func testCancelLoginReturnsToSignedOut() async {
        let sessionStore = AppSessionStore()
        let model = AuthViewModel(sessionStore: sessionStore)

        await model.startFeishuLogin()
        model.cancelLogin()

        XCTAssertNil(model.qrSession)
        XCTAssertEqual(model.statusText, "等待开始")
        XCTAssertFalse(sessionStore.isAuthenticated)
    }
}
