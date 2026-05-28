import SwiftUI

struct FeishuQRCodeLoginView: View {
    @Bindable var model: AuthViewModel

    var body: some View {
        ZStack {
            LoginBackdrop()

            VStack(alignment: .leading, spacing: 22) {
                VStack(alignment: .leading, spacing: 7) {
                    Text("OAR")
                        .font(.codexDisplay(30, weight: .semibold))
                    Text("用飞书扫码继续")
                        .font(.codexBody(15, weight: .semibold))
                        .foregroundStyle(Color.codexMuted)
                }

                QRPanel(model: model)

                HStack(spacing: 8) {
                    BoundaryDot()
                    Text("客户端只保存 OAR 会话，不保存飞书 token。")
                        .font(.codexBody(12, weight: .medium))
                        .foregroundStyle(Color.codexMuted)
                }
            }
            .padding(26)
            .frame(width: 420, alignment: .leading)
            .background(.thinMaterial)
            .background(Color.white.opacity(0.28))
            .overlay(
                RoundedRectangle(cornerRadius: 10)
                    .stroke(Color.white.opacity(0.44), lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: 10))
        }
        .foregroundStyle(Color.codexInk)
    }
}

private struct LoginBackdrop: View {
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
                .fill(Color.white.opacity(0.32))
                .frame(width: 440, height: 440)
                .blur(radius: 58)
                .offset(x: 130, y: -180)
        }
        .ignoresSafeArea()
    }
}

private struct QRPanel: View {
    @Bindable var model: AuthViewModel

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                Label(model.statusText, systemImage: "qrcode.viewfinder")
                    .font(.codexBody(12, weight: .semibold))
                    .foregroundStyle(Color.codexMuted)
                Spacer()
                if model.isWorking || model.transportState == .sseConnecting || model.transportState == .sseLive {
                    ProgressView()
                        .controlSize(.small)
                }
            }

            QRPlaceholder(active: model.qrSession != nil)
                .frame(maxWidth: .infinity)

            if let errorMessage = model.errorMessage {
                Text(errorMessage)
                    .font(.codexBody(12, weight: .semibold))
                    .foregroundStyle(Color.oarSignal)
                    .lineLimit(2)
            }

            HStack(spacing: 10) {
                if model.qrSession == nil {
                    Button {
                        Task {
                            await model.startFeishuLogin()
                        }
                    } label: {
                        Label("开始扫码", systemImage: "qrcode")
                    }
                    .buttonStyle(OARButtonStyle(prominent: true))
                    .disabled(model.isWorking)
                } else {
                    Button {
                        Task {
                            await model.pollOnce()
                        }
                    } label: {
                        Label("刷新状态", systemImage: "arrow.clockwise")
                    }
                    .buttonStyle(OARButtonStyle(prominent: true))
                    .disabled(model.isWorking)

                    Button("取消") {
                        model.cancelLogin()
                    }
                    .buttonStyle(OARButtonStyle(prominent: false))
                    .disabled(model.isWorking)
                }
            }
        }
    }
}

private struct QRPlaceholder: View {
    let active: Bool

    var body: some View {
        VStack(spacing: 12) {
            ZStack {
                RoundedRectangle(cornerRadius: 8)
                    .fill(Color.white.opacity(0.54))
                    .frame(width: 184, height: 184)

                Grid(horizontalSpacing: 8, verticalSpacing: 8) {
                    ForEach(0..<5, id: \.self) { row in
                        GridRow {
                            ForEach(0..<5, id: \.self) { column in
                                RoundedRectangle(cornerRadius: 3)
                                    .fill(tileColor(row: row, column: column))
                                    .frame(width: 22, height: 22)
                            }
                        }
                    }
                }
            }

            Text(active ? "请在飞书中确认授权" : "二维码将在这里显示")
                .font(.codexBody(12, weight: .semibold))
                .foregroundStyle(Color.codexMuted)
        }
    }

    private func tileColor(row: Int, column: Int) -> Color {
        guard active else { return Color.codexMuted.opacity(0.16) }
        let filled = (row + column).isMultiple(of: 2) || row == 0 || column == 4
        return filled ? Color.codexInk.opacity(0.82) : Color.codexMuted.opacity(0.12)
    }
}

private struct BoundaryDot: View {
    var body: some View {
        Circle()
            .fill(Color.oarMoss)
            .frame(width: 7, height: 7)
    }
}
