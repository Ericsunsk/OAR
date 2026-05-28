import XCTest
@testable import OARReviewInbox

final class AuthProviderFactoryTests: XCTestCase {
    func testFactoryDoesNotFallbackToMockWithoutExplicitFlag() {
        let provider = AuthProviderFactory.makeDefaultProvider(
            environment: AppEnvironment(oarBackendBaseURL: nil, allowsMockReviewInboxFallback: false)
        )

        XCTAssertTrue(provider is MissingBackendAuthProvider)
    }

    func testFactoryAllowsMockOnlyWhenExplicitlyEnabled() {
        let provider = AuthProviderFactory.makeDefaultProvider(
            environment: AppEnvironment(
                oarBackendBaseURL: nil,
                allowsMockAuthFallback: true,
                allowsMockReviewInboxFallback: false
            )
        )

        XCTAssertTrue(provider is MockAuthProvider)
    }

    func testFactoryUsesRemoteProviderWhenBaseURLIsConfigured() {
        let provider = AuthProviderFactory.makeDefaultProvider(
            environment: AppEnvironment(
                oarBackendBaseURL: URL(string: "https://oar.example.test")!,
                allowsMockAuthFallback: false,
                allowsMockReviewInboxFallback: false
            )
        )

        XCTAssertTrue(provider is RemoteAuthProvider)
    }
}
