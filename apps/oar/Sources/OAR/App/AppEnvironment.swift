import Foundation

struct AppEnvironment {
    static let defaultBackendBaseURL = URL(string: "https://seven-eels-do.loca.lt")!

    let oarBackendBaseURL: URL?
    let allowsMockAuthFallback: Bool
    let allowsMockReviewInboxFallback: Bool
    let allowsMockAgentFallback: Bool

    init(
        oarBackendBaseURL: URL?,
        allowsMockAuthFallback: Bool = false,
        allowsMockReviewInboxFallback: Bool = false,
        allowsMockAgentFallback: Bool = false
    ) {
        self.oarBackendBaseURL = oarBackendBaseURL
        self.allowsMockAuthFallback = allowsMockAuthFallback
        self.allowsMockReviewInboxFallback = allowsMockReviewInboxFallback
        self.allowsMockAgentFallback = allowsMockAgentFallback
    }

    static func current() -> AppEnvironment {
        AppEnvironment(
            oarBackendBaseURL: defaultBackendBaseURL,
            allowsMockAuthFallback: false,
            allowsMockReviewInboxFallback: false
        )
    }
}
