import SwiftUI

struct DetailHeader: View {
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

struct PrimaryActionPanel: View {
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
        case .draft, .superseded, .withdrawn: .codexMuted
        }
    }
}

struct DetailSection<Content: View>: View {
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

struct EvidenceRow: View {
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

struct AuditRail: View {
    let events: [ReviewInboxTimelineEvent]

    var body: some View {
        HStack(spacing: 9) {
            ForEach(Array(events.enumerated()), id: \.offset) { index, event in
                HStack(spacing: 7) {
                    OARSymbolDot(color: color(for: event.stageStatus), size: 8)
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

struct SafetyBoundary: View {
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

struct ConfirmationDock: View {
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
