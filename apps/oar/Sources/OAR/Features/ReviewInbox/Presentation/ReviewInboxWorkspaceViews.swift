import SwiftUI

struct MainReviewSurface: View {
    @Bindable var model: ReviewInboxViewModel
    @Binding var showAgent: Bool

    var body: some View {
        ReviewWorkspace(model: model, showAgent: $showAgent)
        .background(.regularMaterial)
        .background(Color.codexCanvas.opacity(0.76))
    }
}

private struct ReviewWorkspace: View {
    @Bindable var model: ReviewInboxViewModel
    @Binding var showAgent: Bool

    var body: some View {
        VStack(spacing: 0) {
            WorkspaceToolbar(model: model, showAgent: $showAgent)

            if let item = model.selectedItem {
                ScrollView {
                    VStack(alignment: .leading, spacing: 22) {
                        if let message = model.lastErrorMessage {
                            ErrorBanner(message: message)
                        }

                        RiskStrip(model: model)
                        DetailHeader(item: item)

                        if model.actionsForSelectedItem.count > 1 {
                            ActionChooser(model: model)
                        }

                        if let action = model.selectedAction {
                            PrimaryActionPanel(action: action)
                        }

                        DetailSection("证据摘要") {
                            VStack(spacing: 10) {
                                ForEach(model.evidenceForSelectedItem.prefix(3)) { evidence in
                                    EvidenceRow(evidence: evidence)
                                }
                            }
                        }

                        if let action = model.selectedAction {
                            DetailSection("审计链路") {
                                AuditRail(events: model.ledgerForSelectedAction)
                            }

                            DetailSection("安全边界") {
                                SafetyBoundary()
                            }

                            ConfirmationDock(model: model, action: action)
                        }
                    }
                    .padding(.horizontal, 30)
                    .padding(.top, 24)
                    .padding(.bottom, 28)
                    .frame(maxWidth: .infinity, alignment: .top)
                }
                .scrollIndicators(.hidden)
            } else if model.loadState == .loading {
                LoadingStateView()
                    .padding(28)
            } else if case let .failed(message) = model.loadState {
                ErrorStateView(message: message) {
                    Task {
                        await model.load()
                    }
                }
                .padding(28)
            } else {
                EmptyStateView(title: "暂无待处理项", detail: "当前筛选下没有风险。")
                    .padding(28)
            }
        }
    }
}

private struct WorkspaceToolbar: View {
    @Bindable var model: ReviewInboxViewModel
    @Binding var showAgent: Bool

    var body: some View {
        HStack(spacing: 14) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("本周风险")
                        .font(.codexDisplay(20, weight: .semibold))
                    Text("\(model.filter.rawValue) · \(model.selectedItemPositionText)")
                        .font(.codexBody(12, weight: .semibold))
                        .foregroundStyle(Color.codexMuted)
                }
                Spacer()
                Text("\(model.visibleItemCount)")
                    .font(.system(size: 13, weight: .semibold, design: .monospaced))
                    .foregroundStyle(Color.codexMuted)
            }
            .frame(minWidth: 150, maxWidth: 210, alignment: .leading)

            Picker("筛选", selection: Binding(
                get: { model.filter },
                set: { model.setFilter($0) }
            )) {
                ForEach(ReviewInboxFilter.allCases) { filter in
                    Text(filter.rawValue).tag(filter)
                }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .frame(width: 224)

            Spacer()

            StatusChip(text: "0.6", icon: "hammer")
            StatusChip(text: loadStateText, icon: loadStateIcon)

            Button {
                withAnimation(.easeInOut(duration: 0.2)) {
                    showAgent.toggle()
                }
            } label: {
                Image(systemName: showAgent ? "sidebar.right" : "bubble.left.and.bubble.right")
                    .font(.system(size: 15, weight: .semibold))
                    .frame(width: 30, height: 30)
            }
            .buttonStyle(.plain)
            .foregroundStyle(Color.codexMuted)
            .accessibilityLabel(showAgent ? "隐藏右侧 Agent 栏" : "显示右侧 Agent 栏")
        }
        .padding(.horizontal, 24)
        .padding(.vertical, 18)
        .background(.thinMaterial)
        .background(Color.white.opacity(0.16))
    }

    private var loadStateText: String {
        switch model.loadState {
        case .idle:
            return "待同步"
        case .loading:
            return "同步中"
        case .ready:
            return "已同步"
        case .failed:
            return "同步失败"
        }
    }

    private var loadStateIcon: String {
        switch model.loadState {
        case .idle: "clock"
        case .loading: "arrow.clockwise"
        case .ready: "checkmark"
        case .failed: "exclamationmark.triangle"
        }
    }
}

private struct RiskStrip: View {
    @Bindable var model: ReviewInboxViewModel

    var body: some View {
        ScrollView(.horizontal) {
            HStack(spacing: 10) {
                ForEach(model.sortedItems) { item in
                    Button {
                        model.select(item)
                    } label: {
                        RiskPillCard(
                            item: item,
                            selected: model.selectedItem?.id == item.id
                        )
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("\(item.riskLevel.rawValue)风险：\(item.keyResultTitle)")
                }
            }
        }
        .scrollIndicators(.hidden)
    }
}

private struct RiskPillCard: View {
    let item: ReviewInboxDisplayItem
    let selected: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            HStack {
                HStack(spacing: 7) {
                    OARSymbolDot(color: item.riskLevel.color)
                    Text(item.riskLevel.rawValue)
                        .font(.codexBody(11, weight: .semibold))
                        .foregroundStyle(item.riskLevel.color)
                }
                Spacer()
                Text(item.status.rawValue)
                    .font(.codexBody(11, weight: .semibold))
                    .foregroundStyle(selected ? Color.white.opacity(0.78) : Color.codexMuted)
            }

            Text(item.keyResultTitle)
                .font(.codexBody(13, weight: .semibold))
                .lineLimit(1)

            HStack {
                Text(item.ownerName)
                Spacer()
                Text("可信 \(Int(item.confidenceScore * 100))%")
            }
            .font(.codexBody(11, weight: .medium))
            .foregroundStyle(selected ? Color.white.opacity(0.68) : Color.codexMuted)
        }
        .padding(11)
        .frame(width: 214, height: 92, alignment: .leading)
        .background(selected ? Color.codexInk.opacity(0.88) : Color.white.opacity(0.42))
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(Color.white.opacity(selected ? 0.14 : 0.44), lineWidth: 1)
        )
        .foregroundStyle(selected ? Color.white : Color.codexInk)
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}

private struct StatusChip: View {
    let text: String
    let icon: String

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: icon)
            Text(text)
        }
        .font(.codexBody(11, weight: .semibold))
        .foregroundStyle(Color.codexMuted)
        .padding(.horizontal, 9)
        .frame(height: 26)
        .background(Color.white.opacity(0.42))
        .clipShape(Capsule())
    }
}

private struct ErrorBanner: View {
    let message: String

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "exclamationmark.triangle")
            Text(message)
                .lineLimit(2)
            Spacer()
        }
        .font(.codexBody(12, weight: .semibold))
        .foregroundStyle(Color.oarSignal)
        .padding(10)
        .background(Color.white.opacity(0.48))
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}

private struct LoadingStateView: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            ProgressView()
                .controlSize(.small)
            Text("正在同步复盘收件箱")
                .font(.codexBody(15, weight: .semibold))
            Text("生产客户端会在这里读取后端的 ReviewInboxItem、ProposedAction、Evidence 和 AuditEvent。")
                .font(.codexBody(12))
                .foregroundStyle(Color.codexMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}

private struct ErrorStateView: View {
    let message: String
    let retry: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Image(systemName: "exclamationmark.triangle")
                .font(.title2)
                .foregroundStyle(Color.oarSignal)
            Text("收件箱同步失败")
                .font(.codexBody(15, weight: .semibold))
            Text(message)
                .font(.codexBody(12))
                .foregroundStyle(Color.codexMuted)
            Button("重试", action: retry)
                .buttonStyle(OARButtonStyle(prominent: true))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}

private struct EmptyStateView: View {
    let title: String
    let detail: String

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Image(systemName: "tray")
                .font(.title2)
                .foregroundStyle(Color.codexMuted)
            Text(title)
                .font(.codexBody(15, weight: .semibold))
            Text(detail)
                .font(.codexBody(12))
                .foregroundStyle(Color.codexMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}
