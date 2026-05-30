import SwiftUI

struct ActionChooser: View {
    @Bindable var model: ReviewInboxViewModel

    var body: some View {
        HStack(spacing: 8) {
            ForEach(model.actionsForSelectedItem) { action in
                Button {
                    model.selectAction(action)
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: action.actionType.systemImageName)
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
}

struct PrimaryActionPanel: View {
    let action: ReviewInboxSuggestedAction

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(spacing: 10) {
                Image(systemName: action.actionType.systemImageName)
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
                    .foregroundStyle(action.gateState.tint)
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
}

struct AuditRail: View {
    let events: [ReviewInboxTimelineEvent]

    var body: some View {
        if events.isEmpty {
            HStack(spacing: 8) {
                Image(systemName: "clock.badge.questionmark")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(Color.codexMuted)
                Text("暂无审计链路")
                    .font(.codexBody(12, weight: .semibold))
                    .foregroundStyle(Color.codexMuted)
            }
            .padding(.vertical, 2)
        } else {
            VStack(alignment: .leading, spacing: 8) {
                ForEach(Array(events.enumerated()), id: \.offset) { _, event in
                    AuditRailRow(
                        event: event,
                        color: color(for: event.stageStatus)
                    )
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

private struct AuditRailRow: View {
    let event: ReviewInboxTimelineEvent
    let color: Color

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            OARSymbolDot(color: color, size: 9)
                .padding(.top, 13)
                .frame(width: 12)

            VStack(alignment: .leading, spacing: 5) {
                HStack(spacing: 8) {
                    Image(systemName: icon)
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(Color.codexMuted)
                        .frame(width: 15)
                    Text(event.stage.rawValue)
                        .font(.codexBody(12, weight: .semibold))
                        .foregroundStyle(Color.codexInk)
                    Text(event.stageStatus.rawValue)
                        .font(.codexBody(10, weight: .semibold))
                        .foregroundStyle(color)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(color.opacity(0.12))
                        .clipShape(RoundedRectangle(cornerRadius: 6))
                    Spacer(minLength: 8)
                    Text(event.timestamp)
                        .font(.codexBody(11, weight: .medium))
                        .foregroundStyle(Color.codexMuted)
                        .lineLimit(1)
                }

                Text(event.message)
                    .font(.codexBody(12))
                    .foregroundStyle(Color.codexInk.opacity(0.82))
                    .lineLimit(2)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(.vertical, 8)
            .padding(.horizontal, 10)
            .background(Color.white.opacity(0.34))
            .clipShape(RoundedRectangle(cornerRadius: 8))
            .help("关联键 \(event.idempotencyKey)")
        }
    }

    private var icon: String {
        switch event.stage {
        case .confirmedAction: "person.crop.circle.badge.checkmark"
        case .operationLedger: "list.bullet.clipboard"
        case .larkAdapter: "arrow.triangle.2.circlepath"
        case .auditEvent: "checkmark.seal"
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

private extension ReviewInboxActionType {
    var systemImageName: String {
        switch self {
        case .updateProgress: "pencil.line"
        case .pingOwner: "bell"
        case .createTask: "checkmark.square"
        case .scheduleReview: "calendar.badge.clock"
        }
    }
}

private extension ReviewInboxGateState {
    var tint: Color {
        switch self {
        case .pending: .codexMuted
        case .approved: .oarMoss
        case .rejected: .oarSignal
        case .draft, .superseded, .withdrawn: .codexMuted
        }
    }
}
