import XCTest
@testable import OARReviewInbox

final class ReviewInboxProviderFactoryTests: XCTestCase {
    func testFactoryUsesRemoteProviderWhenBackendBaseURLExists() {
        let provider = ReviewInboxProviderFactory.makeProvider(
            appSession: Self.appSession,
            environment: AppEnvironment(
                oarBackendBaseURL: URL(string: "https://oar.example.test")!,
                allowsMockReviewInboxFallback: false
            )
        )

        XCTAssertTrue(provider is RemoteReviewInboxDataProvider)
    }

    func testFactoryDoesNotFallbackToMockWithoutExplicitFlag() {
        let provider = ReviewInboxProviderFactory.makeProvider(
            appSession: Self.appSession,
            environment: AppEnvironment(
                oarBackendBaseURL: nil,
                allowsMockReviewInboxFallback: false
            )
        )

        XCTAssertTrue(provider is MissingBackendReviewInboxDataProvider)
    }

    func testFactoryAllowsMockOnlyWhenExplicitlyEnabled() {
        let provider = ReviewInboxProviderFactory.makeProvider(
            appSession: Self.appSession,
            environment: AppEnvironment(
                oarBackendBaseURL: nil,
                allowsMockReviewInboxFallback: true
            )
        )

        XCTAssertTrue(provider is MockReviewInboxDataProvider)
    }

    private static let appSession = AppSession(
        sessionID: "oar_session_test",
        user: AuthenticatedUser(
            id: "user_test",
            displayName: "测试用户",
            tenantName: "测试租户"
        )
    )
}
