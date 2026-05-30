import Foundation

@Observable
@MainActor
final class AuthViewModel {
    var state: AuthSessionState = .signedOut
    var transportState: AuthTransportState = .idle
    var isWorking = false
    var errorMessage: String?

    private let provider: FeishuQRCodeAuthProviding
    private let sessionStore: AppSessionStore
    private let pollingIntervalNanoseconds: UInt64
    private var eventTask: Task<Void, Never>?
    private var pollingTask: Task<Void, Never>?

    init(
        provider: FeishuQRCodeAuthProviding,
        sessionStore: AppSessionStore,
        pollingIntervalNanoseconds: UInt64 = 1_000_000_000
    ) {
        self.provider = provider
        self.sessionStore = sessionStore
        self.pollingIntervalNanoseconds = pollingIntervalNanoseconds
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
                return "自动检查中"
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
        cancelLoginObservation()
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
                    guard case let .waitingForScan(session) = self.state,
                          session.id == sessionID else {
                        return
                    }
                    self.errorMessage = "登录事件连接中断，正在自动检查扫码状态。"
                }
            }

            await MainActor.run {
                guard case let .waitingForScan(session) = self.state,
                      session.id == sessionID else {
                    return
                }
                self.transportState = .pollingFallback
                self.startPollingQRCodeSession(sessionID)
            }
        }
    }

    private func startPollingQRCodeSession(_ sessionID: String) {
        pollingTask?.cancel()
        let interval = pollingIntervalNanoseconds

        pollingTask = Task { [provider] in
            while !Task.isCancelled {
                do {
                    try await Task.sleep(nanoseconds: interval)
                } catch is CancellationError {
                    break
                } catch {
                    break
                }

                guard !Task.isCancelled else { break }

                do {
                    let nextState = try await provider.pollFeishuQRCodeSession(sessionID)
                    await MainActor.run {
                        guard case let .waitingForScan(session) = self.state,
                              session.id == sessionID else {
                            return
                        }
                        if self.transportState == .idle {
                            self.transportState = .pollingFallback
                        }
                        self.apply(nextState)
                    }
                } catch is CancellationError {
                    break
                } catch {
                    await MainActor.run {
                        guard case let .waitingForScan(session) = self.state,
                              session.id == sessionID else {
                            return
                        }
                        self.errorMessage = "检查扫码状态失败：\(error.localizedDescription)"
                        self.transportState = .pollingFallback
                    }
                }
            }
        }
    }

    private func apply(_ nextState: AuthSessionState) {
        state = nextState

        switch nextState {
        case let .authorized(appSession):
            sessionStore.apply(appSession)
            cancelLoginObservation()
            transportState = .idle
        case .denied, .expired, .signedOut:
            cancelLoginObservation()
            transportState = .idle
        case .waitingForScan:
            break
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
            cancelLoginObservation()
        case .expired:
            state = .expired
            transportState = .idle
            cancelLoginObservation()
        case .keepalive:
            transportState = .sseLive
        }
    }

    private func cancelLoginObservation() {
        eventTask?.cancel()
        eventTask = nil
        pollingTask?.cancel()
        pollingTask = nil
    }
}
