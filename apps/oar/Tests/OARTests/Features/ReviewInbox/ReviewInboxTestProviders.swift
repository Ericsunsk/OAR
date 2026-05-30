@testable import OAR

struct StaticSnapshotProvider: ReviewInboxDataProviding {
    let snapshot: ReviewInboxDisplaySnapshot

    func loadSnapshot() async throws -> ReviewInboxDisplaySnapshot {
        snapshot
    }

    func submitDecision(
        _ decision: ReviewInboxDecisionCommand,
        snapshot: ReviewInboxDisplaySnapshot
    ) async throws -> ReviewInboxDisplaySnapshot {
        snapshot
    }
}

struct LoadFailingProvider: ReviewInboxDataProviding {
    let error: Error

    func loadSnapshot() async throws -> ReviewInboxDisplaySnapshot {
        throw error
    }

    func submitDecision(
        _ decision: ReviewInboxDecisionCommand,
        snapshot: ReviewInboxDisplaySnapshot
    ) async throws -> ReviewInboxDisplaySnapshot {
        snapshot
    }
}

struct SubmitFailingProvider: ReviewInboxDataProviding {
    let error: Error

    func loadSnapshot() async throws -> ReviewInboxDisplaySnapshot {
        ReviewInboxDisplaySnapshot(
            items: ReviewInboxMockData.reviewItems,
            evidence: ReviewInboxMockData.evidence,
            actions: ReviewInboxMockData.actions,
            ledgerEvents: ReviewInboxMockData.ledgerEvents
        )
    }

    func submitDecision(
        _ decision: ReviewInboxDecisionCommand,
        snapshot: ReviewInboxDisplaySnapshot
    ) async throws -> ReviewInboxDisplaySnapshot {
        throw error
    }
}

actor SequencedLoadProvider: ReviewInboxDataProviding {
    struct Response {
        let delayNanoseconds: UInt64
        let result: Result<ReviewInboxDisplaySnapshot, Error>
    }

    private var responses: [Response]

    init(responses: [Response]) {
        self.responses = responses
    }

    func loadSnapshot() async throws -> ReviewInboxDisplaySnapshot {
        guard !responses.isEmpty else {
            return ReviewInboxDisplaySnapshot(items: [], evidence: [], actions: [], ledgerEvents: [])
        }
        let response = responses.removeFirst()
        try await Task.sleep(nanoseconds: response.delayNanoseconds)
        return try response.result.get()
    }

    func submitDecision(
        _ decision: ReviewInboxDecisionCommand,
        snapshot: ReviewInboxDisplaySnapshot
    ) async throws -> ReviewInboxDisplaySnapshot {
        snapshot
    }
}
