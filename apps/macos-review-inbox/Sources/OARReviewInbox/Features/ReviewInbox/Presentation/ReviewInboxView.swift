import SwiftUI

struct ReviewInboxRootView: View {
    @State private var model = ReviewInboxViewModel()
    @State private var showAgent = true

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
                    AgentSidecar(item: model.selectedItem, action: model.selectedAction)
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
                }
            }
        }
    }
}

private struct ToolbarIconButton: View {
    let systemName: String
    let accessibilityLabel: String
    var isMuted = false
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: systemName)
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(Color.codexMuted.opacity(isMuted ? 0.42 : 0.66))
                .frame(width: 22, height: 22)
        }
        .buttonStyle(.plain)
        .disabled(isMuted)
        .accessibilityLabel(accessibilityLabel)
    }
}

private struct GlassBackdrop: View {
    var body: some View {
        LinearGradient(
            colors: [
                Color(red: 0.96, green: 0.78, blue: 0.56),
                Color(red: 0.77, green: 0.84, blue: 0.95),
                Color(red: 0.91, green: 0.96, blue: 0.88)
            ],
            startPoint: .topLeading,
            endPoint: .bottomTrailing
        )
        .overlay(alignment: .topTrailing) {
            Circle()
                .fill(Color.white.opacity(0.34))
                .frame(width: 440, height: 440)
                .blur(radius: 58)
                .offset(x: 120, y: -170)
        }
        .overlay(alignment: .bottomLeading) {
            RoundedRectangle(cornerRadius: 120)
                .fill(Color.oarMoss.opacity(0.20))
                .frame(width: 520, height: 300)
                .rotationEffect(.degrees(-16))
                .blur(radius: 50)
                .offset(x: -130, y: 80)
        }
        .ignoresSafeArea()
    }
}

private struct NavigationRail: View {
    @Bindable var model: ReviewInboxViewModel

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 8) {
                Text("OAR")
                    .font(.codexDisplay(24, weight: .semibold))
                Text("复盘收件箱")
                    .font(.codexBody(13, weight: .semibold))
                    .foregroundStyle(Color.codexMuted)
            }
            .padding(.top, 92)
            .padding(.horizontal, 22)

            VStack(spacing: 8) {
                NavRow(
                    icon: "tray.full",
                    title: "全部",
                    count: model.items.count,
                    selected: model.filter == .all
                ) {
                    model.setFilter(.all)
                }
                NavRow(
                    icon: "exclamationmark.triangle",
                    title: "高风险",
                    count: model.criticalCount,
                    selected: model.filter == .highRisk
                ) {
                    model.setFilter(.highRisk)
                }
                NavRow(
                    icon: "hand.raised",
                    title: "待确认",
                    count: model.pendingGateCount,
                    selected: model.filter == .needsConfirmation
                ) {
                    model.setFilter(.needsConfirmation)
                }
                NavRow(
                    icon: "checkmark.seal",
                    title: "已执行",
                    count: model.executedCount,
                    selected: model.filter == .executed
                ) {
                    model.setFilter(.executed)
                }
            }
            .padding(.top, 26)
            .padding(.horizontal, 14)

            VStack(alignment: .leading, spacing: 12) {
                Text("当前能力")
                    .font(.codexBody(12, weight: .semibold))
                    .foregroundStyle(Color.codexMuted)

                CapabilityLine(icon: "eye", text: "读取与摘要")
                CapabilityLine(icon: "wand.and.stars", text: "风险诊断")
                CapabilityLine(icon: "doc.text.magnifyingglass", text: "写前预演")
                CapabilityLine(icon: "hand.raised", text: "人工确认")
                CapabilityLine(icon: "lock.doc", text: "审计留痕")
            }
            .padding(.top, 34)
            .padding(.horizontal, 22)

            Spacer()

            HStack(spacing: 8) {
                Circle()
                    .fill(Color.oarAmber)
                    .frame(width: 7, height: 7)
                Text("原型模式")
                    .font(.codexBody(12, weight: .semibold))
                Spacer()
            }
            .foregroundStyle(Color.codexMuted)
            .padding(.horizontal, 22)
            .padding(.bottom, 24)
        }
        .background(.thinMaterial)
        .background(Color.codexSidebar.opacity(0.26))
        .clipped()
    }
}

private struct NavRow: View {
    let icon: String
    let title: String
    var count: Int? = nil
    var selected = false
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 10) {
                Image(systemName: icon)
                    .font(.system(size: 14, weight: .medium))
                    .frame(width: 18)
                Text(title)
                    .font(.codexBody(13, weight: .semibold))
                Spacer()
                if let count {
                    Text("\(count)")
                        .font(.system(size: 10, weight: .bold, design: .monospaced))
                        .padding(.horizontal, 6)
                        .frame(height: 18)
                        .background(selected ? Color.oarMoss : Color.white.opacity(0.45))
                        .foregroundStyle(selected ? Color.white : Color.codexMuted)
                        .clipShape(Capsule())
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .padding(.horizontal, 10)
        .frame(height: 36)
        .background(selected ? Color.white.opacity(0.46) : Color.clear)
        .overlay(
            RoundedRectangle(cornerRadius: 7)
                .stroke(Color.white.opacity(selected ? 0.42 : 0), lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 7))
    }
}

private struct CapabilityLine: View {
    let icon: String
    let text: String

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: icon)
                .font(.system(size: 11, weight: .medium))
                .frame(width: 16)
            Text(text)
                .font(.codexBody(12, weight: .medium))
        }
        .foregroundStyle(Color.codexMuted)
    }
}

private struct MainReviewSurface: View {
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

private struct ActionChooser: View {
    @Bindable var model: ReviewInboxViewModel

    var body: some View {
        HStack(spacing: 8) {
            ForEach(model.actionsForSelectedItem) { action in
                Button {
                    model.selectAction(action)
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: icon(for: action.actionType))
                        Text(action.actionType.rawValue)
                        Text(action.gateState.rawValue)
                            .foregroundStyle(isSelected(action) ? Color.white.opacity(0.64) : Color.codexMuted)
                    }
                    .font(.codexBody(11, weight: .semibold))
                    .padding(.horizontal, 10)
                    .frame(height: 30)
                    .background(isSelected(action) ? Color.codexInk.opacity(0.88) : Color.white.opacity(0.36))
                    .foregroundStyle(isSelected(action) ? Color.white : Color.codexInk)
                    .clipShape(Capsule())
                }
                .buttonStyle(.plain)
            }
        }
    }

    private func isSelected(_ action: ReviewInboxSuggestedAction) -> Bool {
        model.selectedAction?.id == action.id
    }

    private func icon(for actionType: ReviewInboxActionType) -> String {
        switch actionType {
        case .updateProgress: "pencil.line"
        case .pingOwner: "bell"
        case .createTask: "checkmark.square"
        case .scheduleReview: "calendar.badge.clock"
        }
    }
}

private struct RiskStrip: View {
    @Bindable var model: ReviewInboxViewModel

    var body: some View {
        ScrollView(.horizontal) {
            HStack(spacing: 10) {
                ForEach(model.sortedItems) { item in
                    RiskPillCard(
                        item: item,
                        selected: model.selectedItem?.id == item.id
                    )
                    .onTapGesture {
                        model.select(item)
                    }
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
                    Circle()
                        .fill(item.riskLevel.color)
                        .frame(width: 7, height: 7)
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

private struct DetailHeader: View {
    let item: ReviewInboxDisplayItem

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                Text(item.weekLabel)
                Text("·")
                Text(item.ownerName)
                Text("·")
                Text("更新 \(item.lastUpdatedAt)")
            }
            .font(.codexBody(12, weight: .semibold))
            .foregroundStyle(Color.codexMuted)

            Text(item.keyResultTitle)
                .font(.codexDisplay(30, weight: .semibold))
                .lineLimit(2)
                .fixedSize(horizontal: false, vertical: true)

            Text(item.objectiveTitle)
                .font(.codexBody(14, weight: .medium))
                .foregroundStyle(Color.codexMuted)

            Text(item.riskReason)
                .font(.codexBody(15))
                .lineSpacing(4)
                .foregroundStyle(Color.codexInk.opacity(0.86))
        }
    }
}

private struct PrimaryActionPanel: View {
    let action: ReviewInboxSuggestedAction

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(spacing: 10) {
                Image(systemName: actionIcon)
                    .font(.system(size: 15, weight: .semibold))
                    .frame(width: 28, height: 28)
                    .background(Color.codexInk)
                    .foregroundStyle(Color.white)
                    .clipShape(RoundedRectangle(cornerRadius: 7))

                VStack(alignment: .leading, spacing: 3) {
                    Text(action.actionType.rawValue)
                        .font(.codexBody(16, weight: .semibold))
                    Text(action.expectedImpact)
                        .font(.codexBody(12, weight: .medium))
                        .foregroundStyle(Color.codexMuted)
                }

                Spacer()

                Text(action.gateState.rawValue)
                    .font(.codexBody(11, weight: .semibold))
                    .foregroundStyle(gateColor)
            }

            Text(action.rationale)
                .font(.codexBody(13))
                .foregroundStyle(Color.codexInk.opacity(0.86))

            HStack(spacing: 8) {
                Image(systemName: "doc.text.magnifyingglass")
                Text(action.dryRunResultSummary)
            }
            .font(.codexBody(12, weight: .medium))
            .foregroundStyle(Color.codexMuted)
            .padding(10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Color.white.opacity(0.36))
            .clipShape(RoundedRectangle(cornerRadius: 7))
        }
        .padding(15)
        .background(.thinMaterial)
        .background(Color.white.opacity(0.36))
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(Color.white.opacity(0.48), lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    private var actionIcon: String {
        switch action.actionType {
        case .updateProgress: "pencil.line"
        case .pingOwner: "bell"
        case .createTask: "checkmark.square"
        case .scheduleReview: "calendar.badge.clock"
        }
    }

    private var gateColor: Color {
        switch action.gateState {
        case .pending: .codexMuted
        case .approved: .oarMoss
        case .rejected: .oarSignal
        }
    }
}

private struct DetailSection<Content: View>: View {
    let title: String
    let content: Content

    init(_ title: String, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 11) {
            Text(title)
                .font(.codexBody(12, weight: .semibold))
                .foregroundStyle(Color.codexMuted)
            content
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

private struct EvidenceRow: View {
    let evidence: ReviewInboxDisplayEvidence

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: sourceIcon)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(Color.codexMuted)
                .frame(width: 24, height: 24)
                .background(Color.white.opacity(0.42))
                .clipShape(RoundedRectangle(cornerRadius: 6))

            VStack(alignment: .leading, spacing: 4) {
                Text(evidence.summary)
                    .font(.codexBody(13))
                    .lineSpacing(3)

                Text("\(evidence.sourceType.rawValue) · \(evidence.signalType.rawValue) · \(evidence.capturedAt)")
                    .font(.codexBody(11, weight: .medium))
                    .foregroundStyle(Color.codexMuted)
            }

            Spacer()

            Text("\(Int(evidence.trustScore * 100))")
                .font(.system(size: 11, weight: .semibold, design: .monospaced))
                .foregroundStyle(Color.codexMuted)
        }
    }

    private var sourceIcon: String {
        switch evidence.sourceType {
        case .okr: "scope"
        case .task: "checklist"
        case .calendar: "calendar"
        case .meeting: "person.2"
        case .doc: "doc.text"
        case .im: "bubble.left.and.bubble.right"
        }
    }
}

private struct AuditRail: View {
    let events: [ReviewInboxTimelineEvent]

    var body: some View {
        HStack(spacing: 9) {
            ForEach(Array(events.enumerated()), id: \.offset) { index, event in
                HStack(spacing: 7) {
                    Circle()
                        .fill(color(for: event.stageStatus))
                        .frame(width: 8, height: 8)
                    Text(event.stage.rawValue)
                        .font(.codexBody(11, weight: .semibold))
                        .foregroundStyle(event.stageStatus == .pending ? Color.codexMuted : Color.codexInk)
                    if index < events.count - 1 {
                        Image(systemName: "chevron.right")
                            .font(.system(size: 8, weight: .bold))
                            .foregroundStyle(Color.codexMuted.opacity(0.48))
                    }
                }
            }
        }
    }

    private func color(for status: ReviewInboxTimelineStatus) -> Color {
        switch status {
        case .pending: .codexMuted.opacity(0.35)
        case .ok: .oarMoss
        case .error: .oarSignal
        }
    }
}

private struct SafetyBoundary: View {
    var body: some View {
        HStack(spacing: 10) {
            BoundaryItem(icon: "eye", text: "只展示摘要")
            BoundaryItem(icon: "person.crop.circle.badge.checkmark", text: "用户确认")
            BoundaryItem(icon: "repeat", text: "幂等执行")
            BoundaryItem(icon: "lock.doc", text: "审计事件")
        }
    }
}

private struct BoundaryItem: View {
    let icon: String
    let text: String

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: icon)
            Text(text)
        }
        .font(.codexBody(11, weight: .semibold))
        .foregroundStyle(Color.codexMuted)
        .padding(.horizontal, 9)
        .frame(height: 28)
        .background(Color.white.opacity(0.35))
        .clipShape(Capsule())
    }
}

private struct ConfirmationDock: View {
    @Bindable var model: ReviewInboxViewModel
    let action: ReviewInboxSuggestedAction

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if !isExecutableAction {
                HStack(spacing: 8) {
                    Image(systemName: "lock")
                    Text("当前动作仅保留为草稿，MVP 生产入口先开放进展创建 / 更新。")
                }
                .font(.codexBody(11, weight: .semibold))
                .foregroundStyle(Color.codexMuted)
            }

            HStack(spacing: 10) {
                TextField("确认理由或拒绝原因", text: $model.confirmationNote)
                    .font(.codexBody(13))
                    .textFieldStyle(.plain)
                    .padding(.horizontal, 12)
                    .frame(height: 38)
                    .background(Color.white.opacity(0.46))
                    .clipShape(RoundedRectangle(cornerRadius: 8))

                Button {
                    Task {
                        await model.rejectSelectedAction()
                    }
                } label: {
                    Label("拒绝", systemImage: "xmark")
                }
                .buttonStyle(OARButtonStyle(prominent: false))
                .disabled(action.gateState != .pending || model.isSubmittingDecision)

                Button {
                    Task {
                        await model.approveSelectedAction()
                    }
                } label: {
                    Label(model.isSubmittingDecision ? "提交中" : "确认", systemImage: "checkmark")
                }
                .buttonStyle(OARButtonStyle(prominent: true))
                .disabled(!model.canSubmitSelectedAction)
            }
        }
        .padding(12)
        .background(.thinMaterial)
        .background(Color.white.opacity(0.34))
        .clipShape(RoundedRectangle(cornerRadius: 9))
    }

    private var isExecutableAction: Bool {
        action.canEnterProductionExecution
    }
}

private enum AgentRole: Equatable {
    case agent
    case user
}

private struct AgentMessage: Identifiable {
    let id = UUID()
    let role: AgentRole
    let text: String
}

private struct AgentSidecar: View {
    let item: ReviewInboxDisplayItem?
    let action: ReviewInboxSuggestedAction?

    @State private var draft = ""
    @State private var messages = [
        AgentMessage(
            role: .agent,
            text: "我只基于当前风险、摘要证据和 dry-run 结果回答。确认前不会写回飞书。"
        )
    ]

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                Text("OAR Agent")
                    .font(.codexDisplay(16, weight: .semibold))
                Circle()
                    .fill(Color.oarMoss)
                    .frame(width: 6, height: 6)
                Spacer()
            }
            .padding(16)

            ContextCard(item: item, action: action)
                .padding(.horizontal, 16)
                .padding(.bottom, 12)

            Divider()
                .overlay(Color.codexBorder.opacity(0.28))

            ScrollView {
                LazyVStack(spacing: 10) {
                    ForEach(messages) { message in
                        AgentBubble(message: message)
                    }
                }
                .padding(16)
            }
            .scrollIndicators(.hidden)

            ChatInputBar(draft: $draft, send: sendMessage)
        }
        .background(.thinMaterial)
        .background(Color.white.opacity(0.16))
    }

    private func sendMessage() {
        let text = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }
        messages.append(AgentMessage(role: .user, text: text))
        draft = ""
        messages.append(AgentMessage(role: .agent, text: agentReply(for: text)))
    }

    private func agentReply(for text: String) -> String {
        guard let item else {
            return "先选一条风险，我会围绕当前 KR 和摘要证据回答。"
        }

        let actionName = action?.actionType.rawValue ?? "建议动作"
        if text.contains("证据") {
            return "当前证据能解释风险，但仍建议确认负责人最新口径。风险点是：\(item.riskReason)"
        }
        if text.contains("理由") || text.contains("备注") {
            return "可以写：已核对摘要证据和 dry-run 影响范围，同意先执行“\(actionName)”，不修改 owner、target 或权重。"
        }
        return "这条 KR 适合先处理“\(actionName)”。我会保持只读辅助，最终写回必须由你确认。"
    }
}

private struct ContextCard: View {
    let item: ReviewInboxDisplayItem?
    let action: ReviewInboxSuggestedAction?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("当前上下文")
                .font(.codexBody(11, weight: .semibold))
                .foregroundStyle(Color.codexMuted)
            Text(item?.keyResultTitle ?? "未选择风险")
                .font(.codexBody(13, weight: .semibold))
                .lineLimit(2)
            Text(action?.actionType.rawValue ?? "等待建议动作")
                .font(.codexBody(12, weight: .medium))
                .foregroundStyle(Color.codexMuted)
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.white.opacity(0.38))
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}

private struct AgentBubble: View {
    let message: AgentMessage

    private var isUser: Bool {
        message.role == .user
    }

    var body: some View {
        HStack {
            if isUser {
                Spacer(minLength: 34)
            }

            Text(message.text)
                .font(.codexBody(12.5))
                .lineSpacing(3)
                .foregroundStyle(isUser ? Color.white : Color.codexInk)
                .padding(.horizontal, 11)
                .padding(.vertical, 9)
                .background(isUser ? Color.codexInk.opacity(0.88) : Color.white.opacity(0.48))
                .clipShape(RoundedRectangle(cornerRadius: 8))
                .textSelection(.enabled)

            if !isUser {
                Spacer(minLength: 34)
            }
        }
    }
}

private struct ChatInputBar: View {
    @Binding var draft: String
    let send: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            TextField("问证据、理由或风险", text: $draft)
                .font(.codexBody(13))
                .textFieldStyle(.plain)
                .onSubmit(send)

            Button(action: send) {
                Image(systemName: "arrow.up")
                    .font(.system(size: 11, weight: .bold))
                    .frame(width: 25, height: 25)
                    .background(draft.isEmpty ? Color.codexMuted.opacity(0.14) : Color.codexInk)
                    .foregroundStyle(draft.isEmpty ? Color.codexMuted : Color.white)
                    .clipShape(Circle())
            }
            .buttonStyle(.plain)
            .disabled(draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
        }
        .padding(.horizontal, 12)
        .frame(height: 44)
        .background(Color.white.opacity(0.42))
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
