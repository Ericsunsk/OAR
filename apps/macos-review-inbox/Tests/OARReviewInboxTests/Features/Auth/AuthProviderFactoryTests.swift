import XCTest
@testable import OARReviewInbox

final class AuthProviderFactoryTests: XCTestCase {
    func testFactoryDefaultsToMockProvider() {
        let provider = AuthProviderFactory.makeDefaultProvider(environment: [:])

        XCTAssertTrue(provider is MockAuthProvider)
    }

    func testFactoryUsesRemoteProviderWhenBaseURLIsConfigured() {
        let provider = AuthProviderFactory.makeDefaultProvider(
            environment: ["OAR_AUTH_BASE_URL": "https://oar.example.test"]
        )

        XCTAssertTrue(provider is RemoteAuthProvider)
    }
}
