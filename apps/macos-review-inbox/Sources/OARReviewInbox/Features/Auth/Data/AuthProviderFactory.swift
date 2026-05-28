import Foundation

enum AuthProviderFactory {
    static func makeDefaultProvider(environment: [String: String] = ProcessInfo.processInfo.environment) -> AuthProviding {
        guard let rawBaseURL = environment["OAR_AUTH_BASE_URL"],
              let baseURL = URL(string: rawBaseURL) else {
            return MockAuthProvider()
        }

        return RemoteAuthProvider(baseURL: baseURL)
    }
}
