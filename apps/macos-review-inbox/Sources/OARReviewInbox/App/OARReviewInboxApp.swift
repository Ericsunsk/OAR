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

    var body: some View {
        let base = AppRootView(sessionStore: sessionStore)
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

    var body: some View {
        if sessionStore.isAuthenticated {
            ReviewInboxRootView()
        } else {
            FeishuQRCodeLoginView(
                model: AuthViewModel(
                    provider: AuthProviderFactory.makeDefaultProvider(),
                    sessionStore: sessionStore
                )
            )
        }
    }
}
