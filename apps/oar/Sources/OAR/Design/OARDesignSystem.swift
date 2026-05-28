import SwiftUI

extension Color {
    static let oarInk = Color(red: 0.075, green: 0.077, blue: 0.078)
    static let oarPanel = Color(red: 0.965, green: 0.965, blue: 0.955)
    static let oarPanelRaised = Color(red: 0.995, green: 0.995, blue: 0.985)
    static let oarLine = Color(red: 0.74, green: 0.74, blue: 0.70)
    static let oarMuted = Color(red: 0.43, green: 0.43, blue: 0.40)
    static let oarSignal = Color(red: 0.74, green: 0.12, blue: 0.10)
    static let oarRust = Color(red: 0.68, green: 0.30, blue: 0.16)
    static let oarAmber = Color(red: 0.70, green: 0.50, blue: 0.12)
    static let oarMoss = Color(red: 0.18, green: 0.43, blue: 0.29)
    static let oarSteel = Color(red: 0.23, green: 0.25, blue: 0.25)
    static let codexInk = Color(red: 0.12, green: 0.12, blue: 0.13)
    static let codexMuted = Color(red: 0.55, green: 0.55, blue: 0.55)
    static let codexCanvas = Color(red: 0.985, green: 0.985, blue: 0.975)
    static let codexSidebar = Color(red: 0.84, green: 0.88, blue: 0.95).opacity(0.74)
    static let codexSidebarText = Color(red: 0.25, green: 0.25, blue: 0.26)
    static let codexInput = Color(red: 0.995, green: 0.995, blue: 0.99)
    static let codexProjectBar = Color(red: 0.965, green: 0.965, blue: 0.955)
    static let codexSoftControl = Color(red: 0.94, green: 0.94, blue: 0.935)
    static let codexBorder = Color(red: 0.86, green: 0.86, blue: 0.85)
    static let codexSend = Color(red: 0.50, green: 0.50, blue: 0.50)
}

extension Font {
    static func oarDisplay(_ size: CGFloat, weight: Font.Weight = .semibold) -> Font {
        .custom("PingFang SC", size: size).weight(weight)
    }

    static func oarBody(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .custom("PingFang SC", size: size).weight(weight)
    }

    static func codexDisplay(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .custom("PingFang SC", size: size).weight(weight)
    }

    static func codexBody(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .custom("PingFang SC", size: size).weight(weight)
    }
}

struct OARButtonStyle: ButtonStyle {
    let prominent: Bool

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.oarBody(12, weight: .semibold))
            .foregroundStyle(prominent ? Color.oarPanelRaised : Color.oarInk)
            .padding(.horizontal, 12)
            .frame(height: 34)
            .background(prominent ? Color.oarInk : Color.clear)
            .overlay(
                RoundedRectangle(cornerRadius: 6)
                    .stroke(Color.oarLine.opacity(prominent ? 0 : 0.35), lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: 6))
            .opacity(configuration.isPressed ? 0.72 : 1)
    }
}

struct SeverityPill: View {
    let level: ReviewInboxRiskLevel

    var body: some View {
        Text(level.rawValue.uppercased())
            .font(.oarBody(10, weight: .bold))
            .tracking(0)
            .foregroundStyle(Color.oarPanelRaised)
            .padding(.horizontal, 8)
            .frame(height: 22)
            .background(level.color)
            .clipShape(RoundedRectangle(cornerRadius: 4))
    }
}

struct StatusBadge: View {
    let status: ReviewInboxDisplayStatus

    var body: some View {
        Label(status.rawValue, systemImage: symbol)
            .font(.oarBody(11, weight: .semibold))
            .foregroundStyle(color)
            .labelStyle(.titleAndIcon)
    }

    private var symbol: String {
        switch status {
        case .new: "tray"
        case .needsConfirmation: "hand.raised"
        case .confirmed: "checkmark.seal"
        case .executed: "bolt.horizontal"
        case .failed: "exclamationmark.triangle"
        case .rejected: "xmark.seal"
        }
    }

    private var color: Color {
        switch status {
        case .new: .oarMuted
        case .needsConfirmation: .oarAmber
        case .confirmed: .oarSteel
        case .executed: .oarMoss
        case .failed: .oarSignal
        case .rejected: .oarMuted
        }
    }
}

struct MetricTile: View {
    let value: String
    let label: String
    let tint: Color

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(value)
                .font(.oarDisplay(28, weight: .bold))
                .foregroundStyle(tint)
            Text(label.uppercased())
                .font(.oarBody(10, weight: .bold))
                .tracking(0)
                .foregroundStyle(Color.oarMuted)
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.oarPanelRaised)
        .overlay(alignment: .leading) {
            Rectangle()
                .fill(tint)
                .frame(width: 3)
        }
        .clipShape(RoundedRectangle(cornerRadius: 6))
    }
}
