import SwiftUI

struct ToolbarIconButton: View {
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

struct GlassBackdrop: View {
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

struct NavigationRail: View {
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
                ForEach(ReviewInboxFilter.allCases) { filter in
                    NavRow(
                        icon: filter.navigationIconName,
                        title: filter.rawValue,
                        count: model.count(for: filter),
                        selected: model.filter == filter
                    ) {
                        model.setFilter(filter)
                    }
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
                OARSymbolDot(color: Color.oarAmber)
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

private extension ReviewInboxFilter {
    var navigationIconName: String {
        switch self {
        case .all: "tray.full"
        case .highRisk: "exclamationmark.triangle"
        case .needsConfirmation: "hand.raised"
        case .confirmed: "checkmark.circle"
        case .executing: "clock.arrow.circlepath"
        case .failed: "xmark.octagon"
        case .executed: "checkmark.seal"
        case .cancelled: "minus.circle"
        case .rejected: "xmark.seal"
        }
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
