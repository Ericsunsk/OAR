import Foundation

protocol FeishuQRCodeAuthProviding {
    func createFeishuQRCodeSession() async throws -> FeishuQRCodeAuthSession
    func pollFeishuQRCodeSession(_ sessionID: String) async throws -> AuthSessionState
    func subscribeFeishuQRCodeSession(_ sessionID: String) -> AsyncThrowingStream<AuthLoginEvent, Error>
}

protocol SessionSignOutProviding {
    func signOut(appSession: AppSession) async throws
}

typealias AuthProviding = FeishuQRCodeAuthProviding & SessionSignOutProviding

enum AuthProviderError: LocalizedError {
    case missingBackendConfiguration
    case sessionNotFound
    case loginDenied
    case invalidSession
    case invalidResponse
    case remoteUnavailable

    var errorDescription: String? {
        switch self {
        case .missingBackendConfiguration:
            return "请配置 OAR 后端地址后再使用飞书扫码登录。"
        case .sessionNotFound:
            return "登录会话不存在或已过期。"
        case .loginDenied:
            return "飞书扫码授权已取消。"
        case .invalidSession:
            return "当前 OAR 会话已失效。"
        case .invalidResponse:
            return "登录服务返回了无法识别的响应。"
        case .remoteUnavailable:
            return "登录服务暂时不可用。"
        }
    }
}

final class MockAuthProvider: AuthProviding {
    private var pollCountBySessionID: [String: Int] = [:]

    func createFeishuQRCodeSession() async throws -> FeishuQRCodeAuthSession {
        let sessionID = "mock-feishu-login"
        pollCountBySessionID[sessionID] = 0
        return FeishuQRCodeAuthSession(
            id: sessionID,
            qrPageURL: URL(string: "https://open.feishu.cn/mock-oar-login")!,
            expiresAt: Date().addingTimeInterval(300)
        )
    }

    func pollFeishuQRCodeSession(_ sessionID: String) async throws -> AuthSessionState {
        guard let pollCount = pollCountBySessionID[sessionID] else {
            throw AuthProviderError.sessionNotFound
        }

        let nextCount = pollCount + 1
        pollCountBySessionID[sessionID] = nextCount

        guard nextCount >= 2 else {
            return .waitingForScan(
                FeishuQRCodeAuthSession(
                    id: sessionID,
                    qrPageURL: URL(string: "https://open.feishu.cn/mock-oar-login")!,
                    expiresAt: Date().addingTimeInterval(300)
                )
            )
        }

        return .authorized(
            AppSession(
                sessionID: "mock-oar-session",
                user: AuthenticatedUser(
                    id: "user_mock_chenmin",
                    displayName: "陈敏",
                    tenantName: "OAR 测试租户"
                )
            )
        )
    }

    func subscribeFeishuQRCodeSession(_ sessionID: String) -> AsyncThrowingStream<AuthLoginEvent, Error> {
        AsyncThrowingStream { continuation in
            Task {
                do {
                    let pendingState = try await pollFeishuQRCodeSession(sessionID)
                    if case let .waitingForScan(qrSession) = pendingState {
                        continuation.yield(.pending(sessionID: sessionID, qrSession: qrSession))
                    }

                    let authorizedState = try await pollFeishuQRCodeSession(sessionID)
                    if case let .authorized(appSession) = authorizedState {
                        continuation.yield(.authorized(sessionID: sessionID, appSession: appSession))
                    }
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
        }
    }

    func signOut(appSession: AppSession) async throws {
        pollCountBySessionID.removeAll()
    }
}

struct MissingBackendAuthProvider: AuthProviding {
    func createFeishuQRCodeSession() async throws -> FeishuQRCodeAuthSession {
        throw AuthProviderError.missingBackendConfiguration
    }

    func pollFeishuQRCodeSession(_ sessionID: String) async throws -> AuthSessionState {
        throw AuthProviderError.missingBackendConfiguration
    }

    func subscribeFeishuQRCodeSession(_ sessionID: String) -> AsyncThrowingStream<AuthLoginEvent, Error> {
        AsyncThrowingStream { continuation in
            continuation.finish(throwing: AuthProviderError.missingBackendConfiguration)
        }
    }

    func signOut(appSession: AppSession) async throws {
        throw AuthProviderError.missingBackendConfiguration
    }
}

struct RemoteAuthProvider: AuthProviding {
    let baseURL: URL
    let urlSession: URLSession
    let decoder: JSONDecoder
    let dateParser: ISO8601DateFormatter

    init(
        baseURL: URL,
        urlSession: URLSession = .shared,
        decoder: JSONDecoder = JSONDecoder(),
        dateParser: ISO8601DateFormatter = ISO8601DateFormatter()
    ) {
        self.baseURL = baseURL
        self.urlSession = urlSession
        self.decoder = decoder
        self.dateParser = dateParser
    }

    func createFeishuQRCodeSession() async throws -> FeishuQRCodeAuthSession {
        let endpoint = baseURL.appendingPathComponent("auth/feishu/qr-sessions")
        var request = URLRequest(url: endpoint)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        let data = try await performRequest(request)
        let response = try decoder.decode(CreateFeishuQRCodeSessionResponseDTO.self, from: data)
        return try response.toDomain(dateParser: dateParser)
    }

    func pollFeishuQRCodeSession(_ sessionID: String) async throws -> AuthSessionState {
        let endpoint = baseURL
            .appendingPathComponent("auth/feishu/qr-sessions")
            .appendingPathComponent(sessionID)
        let data = try await performRequest(URLRequest(url: endpoint))
        let response = try decoder.decode(FeishuQRCodeSessionStatusResponseDTO.self, from: data)
        return try response.toDomainState(dateParser: dateParser)
    }

    func subscribeFeishuQRCodeSession(_ sessionID: String) -> AsyncThrowingStream<AuthLoginEvent, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    let endpoint = baseURL
                        .appendingPathComponent("auth/feishu/qr-sessions")
                        .appendingPathComponent(sessionID)
                        .appendingPathComponent("events")
                    var request = URLRequest(url: endpoint)
                    request.setValue("text/event-stream", forHTTPHeaderField: "Accept")

                    let (bytes, response) = try await urlSession.bytes(for: request)
                    guard let httpResponse = response as? HTTPURLResponse,
                          200..<300 ~= httpResponse.statusCode else {
                        throw AuthProviderError.remoteUnavailable
                    }

                    for try await line in bytes.lines {
                        guard let event = try decodeServerSentEventLine(line) else { continue }
                        continuation.yield(event)
                        if event.isTerminalAuthEvent {
                            break
                        }
                    }

                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }

            continuation.onTermination = { _ in
                task.cancel()
            }
        }
    }

    func signOut(appSession: AppSession) async throws {
        let endpoint = baseURL.appendingPathComponent("auth/logout")
        var request = URLRequest(url: endpoint)
        request.httpMethod = "POST"
        request.setValue("Bearer \(appSession.sessionID)", forHTTPHeaderField: "Authorization")
        try await performSignOutRequest(request)
    }

    private func performRequest(_ request: URLRequest) async throws -> Data {
        let (data, response) = try await urlSession.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw AuthProviderError.remoteUnavailable
        }

        switch httpResponse.statusCode {
        case 200..<300:
            return data
        case 404, 410:
            throw AuthProviderError.sessionNotFound
        case 401, 403:
            throw AuthProviderError.loginDenied
        default:
            throw AuthProviderError.remoteUnavailable
        }
    }

    private func performSignOutRequest(_ request: URLRequest) async throws {
        let (_, response) = try await urlSession.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw AuthProviderError.remoteUnavailable
        }

        switch httpResponse.statusCode {
        case 200..<300:
            return
        case 401, 403:
            throw AuthProviderError.invalidSession
        default:
            throw AuthProviderError.remoteUnavailable
        }
    }

    private func decodeServerSentEventLine(_ line: String) throws -> AuthLoginEvent? {
        guard line.hasPrefix("data:") else { return nil }
        let json = line.dropFirst("data:".count).trimmingCharacters(in: .whitespaces)
        guard !json.isEmpty else { return nil }
        let dto = try decoder.decode(AuthLoginEventDTO.self, from: Data(json.utf8))
        return try dto.toDomainEvent(dateParser: dateParser)
    }
}

private extension AuthLoginEvent {
    var isTerminalAuthEvent: Bool {
        switch self {
        case .authorized, .denied, .expired:
            return true
        case .pending, .keepalive:
            return false
        }
    }
}
