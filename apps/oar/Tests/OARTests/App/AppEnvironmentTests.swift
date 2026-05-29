import XCTest
@testable import OAR

final class AppEnvironmentTests: XCTestCase {
    func testCurrentUsesDefaultBackendBaseURLAndMockFlagsOff() {
        let environment = AppEnvironment.current()

        XCTAssertEqual(environment.oarBackendBaseURL, URL(string: "https://seven-eels-do.loca.lt"))
        XCTAssertFalse(environment.allowsMockAuthFallback)
        XCTAssertFalse(environment.allowsMockReviewInboxFallback)
        XCTAssertFalse(environment.allowsMockAgentFallback)
    }

    func testBackendBaseURLCanStillBeInjectedForTestsAndFutureSettings() {
        let environment = AppEnvironment(
            oarBackendBaseURL: URL(string: "https://oar.example.test")!,
            allowsMockAuthFallback: true,
            allowsMockReviewInboxFallback: true,
            allowsMockAgentFallback: true
        )

        XCTAssertEqual(environment.oarBackendBaseURL, URL(string: "https://oar.example.test"))
        XCTAssertTrue(environment.allowsMockAuthFallback)
        XCTAssertTrue(environment.allowsMockReviewInboxFallback)
        XCTAssertTrue(environment.allowsMockAgentFallback)
    }
}
