import Foundation

@Observable
@MainActor
final class AuthViewModel {
    var state: AuthSessionState = .signedOut
    var transportState: AuthTransportState = .idle
    var isWorking = false
    var errorMessage: String?

    private let provider: AuthProviding
    private let sessionStore: AppSessionStore
    private var eventTask: Task<Void, Never>?

    init(provider: AuthProviding = MockAuthProvider(), sessionStore: AppSessionStore) {
        self.provider = provider
        self.sessionStore = sessionStore
    }

    var qrSession: FeishuQRCodeAuthSession? {
        if case let .waitingForScan(session) = state {
            return session
        }
        return nil
    }

    var statusText: String {
        switch state {
        case .signedOut:
            return "等待开始"
        case .waitingForScan:
            switch transportState {
            case .sseConnecting:
                return "连接登录事件"
            case .sseLive:
                return "等待扫码"
            case .pollingFallback:
                return "可手动刷新"
            case .idle:
                return "等待扫码"
            }
        case .authorized:
            return "已登录"
        case .denied:
            return "授权取消"
        case .expired:
            return "二维码过期"
        }
    }

    func startFeishuLogin() async {
        guard !isWorking else { return }
        isWorking = true
        errorMessage = nil

        do {
            let session = try await provider.createFeishuQRCodeSession()
            state = .waitingForScan(session)
            listenForQRCodeSessionEvents(session.id)
        } catch {
            errorMessage = "创建飞书登录会话失败：\(error.localizedDescription)"
            state = .signedOut
        }

        isWorking = false
    }

    func pollOnce() async {
        guard case let .waitingForScan(session) = state else { return }
        guard !isWorking else { return }
        isWorking = true
        errorMessage = nil

        do {
            let nextState = try await provider.pollFeishuQRCodeSession(session.id)
            apply(nextState)
        } catch {
            errorMessage = "检查扫码状态失败：\(error.localizedDescription)"
        }

        isWorking = false
    }

    func cancelLogin() {
        eventTask?.cancel()
        eventTask = nil
        transportState = .idle
        state = .signedOut
        errorMessage = nil
    }

    private func listenForQRCodeSessionEvents(_ sessionID: String) {
        eventTask?.cancel()
        transportState = .sseConnecting

        eventTask = Task { [provider] in
            do {
                for try await event in provider.subscribeFeishuQRCodeSession(sessionID) {
                    await MainActor.run {
                        self.apply(event, expectedSessionID: sessionID)
                    }
                }
            } catch is CancellationError {
            } catch {
                await MainActor.run {
                    self.errorMessage = "登录事件连接中断，可手动刷新扫码状态。"
                    self.transportState = .pollingFallback
                }
            }

            await MainActor.run {
                if self.transportState != .pollingFallback {
                    self.transportState = .idle
                }
            }
        }
    }

    private func apply(_ nextState: AuthSessionState) {
        state = nextState
        if case let .authorized(appSession) = nextState {
            sessionStore.apply(appSession)
            eventTask?.cancel()
            eventTask = nil
            transportState = .idle
        }
    }

    private func apply(_ event: AuthLoginEvent, expectedSessionID: String) {
        guard event.sessionID == expectedSessionID else { return }

        switch event {
        case let .pending(_, qrSession):
            transportState = .sseLive
            state = .waitingForScan(qrSession)
        case let .authorized(_, appSession):
            apply(.authorized(appSession))
        case let .denied(_, message):
            state = .denied(message)
            transportState = .idle
            eventTask?.cancel()
            eventTask = nil
        case .expired:
            state = .expired
            transportState = .idle
            eventTask?.cancel()
            eventTask = nil
        case .keepalive:
            transportState = .sseLive
        }
    }
}
