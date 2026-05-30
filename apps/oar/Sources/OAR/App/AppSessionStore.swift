import Foundation

@Observable
@MainActor
final class AppSessionStore {
    var session: AppSession?
    var sessionTerminationMessage: String?

    var isAuthenticated: Bool {
        session != nil
    }

    func apply(_ session: AppSession) {
        self.session = session
        sessionTerminationMessage = nil
    }

    func clear(reason: String? = nil) {
        session = nil
        let trimmed = reason?.trimmingCharacters(in: .whitespacesAndNewlines)
        sessionTerminationMessage = trimmed?.isEmpty == false ? trimmed : nil
    }

    func clearIfCurrent(sessionID: String, reason: String? = nil) {
        guard session?.sessionID == sessionID else { return }
        clear(reason: reason)
    }

    func dismissSessionTerminationMessage() {
        sessionTerminationMessage = nil
    }
}
