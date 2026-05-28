import SwiftUI

@main
struct OARReviewInboxApp: App {
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
                )
            )
        } else {
            FeishuQRCodeLoginView(
                model: AuthViewModel(
                    provider: AuthProviderFactory.makeDefaultProvider(environment: environment),
                    sessionStore: sessionStore
                )
            )
        }
    }
}
