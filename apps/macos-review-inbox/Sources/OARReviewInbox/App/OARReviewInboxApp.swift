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
    var body: some View {
        let base = ReviewInboxRootView()
            .frame(minWidth: 1360, minHeight: 780)

        if #available(macOS 15.0, *) {
            base.toolbarBackgroundVisibility(.hidden, for: .windowToolbar)
        } else {
            base
        }
    }
}
