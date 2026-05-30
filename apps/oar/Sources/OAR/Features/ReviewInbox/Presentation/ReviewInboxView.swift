import SwiftUI

struct ReviewInboxRootView: View {
    @State private var model: ReviewInboxViewModel
    @State private var agentModel: AgentSidecarViewModel
    @State private var agentSettingsModel: AgentSettingsViewModel
    @State private var showAgent = true
    @State private var isSigningOut = false
    private let onSignOut: @MainActor () async -> Void

    init(
        provider: ReviewInboxDataProviding,
        agentProvider: AgentProviding,
        agentSettingsProvider: AgentSettingsProviding,
        onSessionInvalidated: @escaping @MainActor (String) -> Void = { _ in },
        onSignOut: @escaping @MainActor () async -> Void = {}
    ) {
        _model = State(initialValue: ReviewInboxViewModel(
            provider: provider,
            onSessionInvalidated: onSessionInvalidated
        ))
        _agentModel = State(initialValue: AgentSidecarViewModel(provider: agentProvider))
        _agentSettingsModel = State(initialValue: AgentSettingsViewModel(provider: agentSettingsProvider))
        self.onSignOut = onSignOut
    }

    var body: some View {
        ZStack {
            GlassBackdrop()

            HStack(spacing: 0) {
                NavigationRail(model: model)
                    .frame(width: 260)
                    .layoutPriority(2)

                MainReviewSurface(model: model, showAgent: $showAgent)
                    .frame(minWidth: 620, maxWidth: .infinity)
                    .layoutPriority(1)

                if showAgent {
                    AgentSidecarView(
                        model: agentModel,
                        settingsModel: agentSettingsModel,
                        item: model.selectedItem,
                        action: model.selectedAction,
                        context: model.agentWorkspaceContext
                    )
                        .frame(width: 320)
                        .layoutPriority(2)
                        .transition(.move(edge: .trailing).combined(with: .opacity))
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(.ultraThinMaterial)
        }
        .foregroundStyle(Color.codexInk)
        .task {
            await model.load()
        }
        .toolbar {
            ToolbarItem(placement: .navigation) {
                HStack(spacing: 24) {
                    ToolbarIconButton(
                        systemName: "arrow.clockwise",
                        accessibilityLabel: "刷新",
                        isMuted: model.loadState == .loading
                    ) {
                        Task {
                            await model.reload()
                        }
                    }
                    ToolbarIconButton(
                        systemName: "chevron.left",
                        accessibilityLabel: "上一条",
                        isMuted: !model.canMoveToPreviousItem,
                        action: model.selectPreviousItem
                    )
                    ToolbarIconButton(
                        systemName: "chevron.right",
                        accessibilityLabel: "下一条",
                        isMuted: !model.canMoveToNextItem,
                        action: model.selectNextItem
                    )
                    ToolbarIconButton(
                        systemName: "rectangle.portrait.and.arrow.right",
                        accessibilityLabel: "退出登录",
                        isMuted: isSigningOut
                    ) {
                        Task {
                            await signOut()
                        }
                    }
                }
            }
        }
    }

    private func signOut() async {
        guard !isSigningOut else { return }
        isSigningOut = true
        await onSignOut()
        isSigningOut = false
    }
}
