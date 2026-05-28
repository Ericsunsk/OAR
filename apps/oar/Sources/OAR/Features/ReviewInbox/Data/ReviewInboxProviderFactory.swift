import Foundation

enum ReviewInboxProviderFactory {
    static func makeProvider(
        appSession: AppSession,
        environment: AppEnvironment = .current()
    ) -> ReviewInboxDataProviding {
        if let baseURL = environment.oarBackendBaseURL {
            return RemoteReviewInboxDataProvider(baseURL: baseURL, appSession: appSession)
        }

        if environment.allowsMockReviewInboxFallback {
            return MockReviewInboxDataProvider()
        }

        return MissingBackendReviewInboxDataProvider()
    }
}

struct MissingBackendReviewInboxDataProvider: ReviewInboxDataProviding {
    func loadSnapshot() async throws -> ReviewInboxDisplaySnapshot {
        throw ReviewInboxDataProviderError.missingBackendConfiguration
    }

    func submitDecision(
        _ decision: ReviewInboxDecisionCommand,
        snapshot: ReviewInboxDisplaySnapshot
    ) async throws -> ReviewInboxDisplaySnapshot {
        throw ReviewInboxDataProviderError.missingBackendConfiguration
    }
}
