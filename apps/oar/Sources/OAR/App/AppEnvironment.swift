import Foundation

struct AppEnvironment {
    static let defaultBackendBaseURL = URL(string: "http://127.0.0.1:8080")!

    let oarBackendBaseURL: URL?
    let allowsMockAuthFallback: Bool
    let allowsMockReviewInboxFallback: Bool

    init(
        oarBackendBaseURL: URL?,
        allowsMockAuthFallback: Bool = false,
        allowsMockReviewInboxFallback: Bool = false
    ) {
        self.oarBackendBaseURL = oarBackendBaseURL
        self.allowsMockAuthFallback = allowsMockAuthFallback
        self.allowsMockReviewInboxFallback = allowsMockReviewInboxFallback
    }

    static func current() -> AppEnvironment {
        AppEnvironment(
            oarBackendBaseURL: defaultBackendBaseURL,
            allowsMockAuthFallback: false,
            allowsMockReviewInboxFallback: false
        )
    }
}
