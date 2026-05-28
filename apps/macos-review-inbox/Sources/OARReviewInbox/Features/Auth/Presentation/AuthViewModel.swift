import Foundation

@Observable
@MainActor
final class AuthViewModel {
    var state: AuthSessionState = .signedOut
    var isWorking = false
    var errorMessage: String?

    private let provider: AuthProviding
    private let sessionStore: AppSessionStore

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
            return "等待扫码"
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
            state = .waitingForScan(try await provider.createFeishuQRCodeSession())
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
            state = nextState
            if case let .authorized(appSession) = nextState {
                sessionStore.apply(appSession)
            }
        } catch {
            errorMessage = "检查扫码状态失败：\(error.localizedDescription)"
        }

        isWorking = false
    }

    func cancelLogin() {
        state = .signedOut
        errorMessage = nil
    }
}
