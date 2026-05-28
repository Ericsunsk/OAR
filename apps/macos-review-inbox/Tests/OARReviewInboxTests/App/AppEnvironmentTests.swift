import XCTest
@testable import OARReviewInbox

final class AppEnvironmentTests: XCTestCase {
    func testCurrentReadsBackendBaseURLAndMockFlags() throws {
        let environment = AppEnvironment.current(
            environment: [
                "OAR_BACKEND_BASE_URL": "https://oar.example.test",
                "OAR_ALLOW_MOCK_AUTH": "1",
                "OAR_ALLOW_MOCK_REVIEW_INBOX": "1"
            ]
        )

        XCTAssertEqual(environment.oarBackendBaseURL, URL(string: "https://oar.example.test"))
        XCTAssertTrue(environment.allowsMockAuthFallback)
        XCTAssertTrue(environment.allowsMockReviewInboxFallback)
    }

    func testCurrentDefaultsMockFlagsOff() {
        let environment = AppEnvironment.current(environment: [:])

        XCTAssertNil(environment.oarBackendBaseURL)
        XCTAssertFalse(environment.allowsMockAuthFallback)
        XCTAssertFalse(environment.allowsMockReviewInboxFallback)
    }

    func testCurrentIgnoresInvalidBackendBaseURL() {
        let environment = AppEnvironment.current(
            environment: [
                "OAR_BACKEND_BASE_URL": "localhost:8080"
            ]
        )

        XCTAssertNil(environment.oarBackendBaseURL)
    }
}
