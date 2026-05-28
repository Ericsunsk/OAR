import Foundation

enum AuthProviderFactory {
    static func makeDefaultProvider(environment: AppEnvironment = .current()) -> AuthProviding {
        if let baseURL = environment.oarBackendBaseURL {
            return RemoteAuthProvider(baseURL: baseURL)
        }

        if environment.allowsMockAuthFallback {
            return MockAuthProvider()
        }

        return MissingBackendAuthProvider()
    }
}
