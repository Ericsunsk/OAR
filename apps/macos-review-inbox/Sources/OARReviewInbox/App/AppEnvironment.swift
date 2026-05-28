import Foundation

struct AppEnvironment {
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

    static func current(environment: [String: String] = ProcessInfo.processInfo.environment) -> AppEnvironment {
        AppEnvironment(
            oarBackendBaseURL: backendBaseURL(from: environment["OAR_BACKEND_BASE_URL"]),
            allowsMockAuthFallback: environment["OAR_ALLOW_MOCK_AUTH"] == "1",
            allowsMockReviewInboxFallback: environment["OAR_ALLOW_MOCK_REVIEW_INBOX"] == "1"
        )
    }

    private static func backendBaseURL(from rawValue: String?) -> URL? {
        guard let rawValue,
              let url = URL(string: rawValue),
              let scheme = url.scheme?.lowercased(),
              ["http", "https"].contains(scheme),
              url.host != nil else {
            return nil
        }

        return url
    }
}
