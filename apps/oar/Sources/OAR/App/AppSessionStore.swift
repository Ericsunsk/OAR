import Foundation

@Observable
@MainActor
final class AppSessionStore {
    var session: AppSession?

    var isAuthenticated: Bool {
        session != nil
    }

    func apply(_ session: AppSession) {
        self.session = session
    }

    func clear() {
        session = nil
    }
}
