import SwiftUI

@main
struct OARApp: App {
    var body: some Scene {
        WindowGroup {
            RootWindowView()
        }
        .windowStyle(.hiddenTitleBar)
    }
}

private struct RootWindowView: View {
    @State private var sessionStore = AppSessionStore()
    private let environment = AppEnvironment.current()

    var body: some View {
        let base = AppRootView(sessionStore: sessionStore, environment: environment)
            .frame(minWidth: 1360, minHeight: 780)

        if #available(macOS 15.0, *) {
            base.toolbarBackgroundVisibility(.hidden, for: .windowToolbar)
        } else {
            base
        }
    }
}

private struct AppRootView: View {
    @Bindable var sessionStore: AppSessionStore
    let environment: AppEnvironment

    var body: some View {
        if let session = sessionStore.session {
            ReviewInboxRootView(
                provider: ReviewInboxProviderFactory.makeProvider(
                    appSession: session,
                    environment: environment
                ),
                onSessionInvalidated: { message in
                    sessionStore.clear(reason: message)
                }
            )
        } else {
            ZStack(alignment: .top) {
                FeishuQRCodeLoginView(
                    model: AuthViewModel(
                        provider: AuthProviderFactory.makeDefaultProvider(environment: environment),
                        sessionStore: sessionStore
                    )
                )

                if let message = sessionStore.sessionTerminationMessage {
                    SessionTerminationBanner(message: message) {
                        sessionStore.dismissSessionTerminationMessage()
                    }
                    .padding(.top, 22)
                }
            }
        }
    }
}

private struct SessionTerminationBanner: View {
    let message: String
    let dismiss: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "exclamationmark.triangle.fill")
            Text(message)
                .lineLimit(2)
            Button(action: dismiss) {
                Image(systemName: "xmark")
                    .font(.system(size: 11, weight: .semibold))
                    .frame(width: 18, height: 18)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("关闭会话提示")
        }
        .font(.codexBody(12, weight: .semibold))
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background(.thinMaterial)
        .background(Color.white.opacity(0.35))
        .foregroundStyle(Color.oarSignal)
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}
