import SwiftUI

struct AgentSettingsSheet: View {
    @Bindable var model: AgentSettingsViewModel
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(spacing: 10) {
                Text("Agent 设置")
                    .font(.codexDisplay(18, weight: .semibold))
                OARSymbolDot(color: sourceColor, size: 7)
                Spacer()
                Button {
                    dismiss()
                } label: {
                    Image(systemName: "xmark")
                        .font(.system(size: 11, weight: .semibold))
                        .frame(width: 24, height: 24)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("关闭 Agent 设置")
            }

            VStack(spacing: 10) {
                AgentSettingsField(title: "Base URL") {
                    TextField("https://api.openai.com/v1", text: $model.baseURL)
                        .textFieldStyle(.plain)
                }

                AgentSettingsField(title: "API Key") {
                    SecureField(model.apiKeyPlaceholder, text: $model.apiKey)
                        .textFieldStyle(.plain)
                }

                HStack(spacing: 8) {
                    AgentSettingsPill(title: protocolTitle)
                    AgentSettingsPill(title: sourceTitle)
                    Spacer()
                    Button {
                        Task { await model.detect() }
                    } label: {
                        Image(systemName: model.isDetecting ? "hourglass" : "waveform.path.ecg")
                            .font(.system(size: 12, weight: .semibold))
                            .frame(width: 28, height: 28)
                    }
                    .buttonStyle(.plain)
                    .disabled(!model.canDetect)
                    .accessibilityLabel("检测模型")
                }

                AgentSettingsField(title: "Model") {
                    Picker("", selection: $model.selectedModelID) {
                        if model.models.isEmpty {
                            Text(model.selectedModelID.isEmpty ? "未检测" : model.selectedModelID)
                                .tag(model.selectedModelID)
                        } else {
                            ForEach(model.models) { candidate in
                                Text(candidate.displayName).tag(candidate.id)
                            }
                        }
                    }
                    .labelsHidden()
                    .pickerStyle(.menu)
                    .disabled(model.models.isEmpty)
                }
            }

            if let message = model.errorMessage {
                AgentSettingsMessage(message: message, color: Color.oarSignal)
            } else if let message = model.statusMessage {
                AgentSettingsMessage(message: message, color: Color.oarMoss)
            }

            HStack(spacing: 9) {
                Button {
                    Task { await model.clear() }
                } label: {
                    Image(systemName: "trash")
                        .font(.system(size: 12, weight: .semibold))
                        .frame(width: 30, height: 30)
                }
                .buttonStyle(.plain)
                .disabled(!model.canConfigure || model.isSaving)
                .accessibilityLabel("清除 Agent 设置")

                Spacer()

                Button("保存") {
                    Task { await model.save() }
                }
                .font(.codexBody(12.5, weight: .semibold))
                .buttonStyle(.plain)
                .padding(.horizontal, 14)
                .frame(height: 30)
                .background(model.canSave ? Color.codexInk : Color.codexMuted.opacity(0.16))
                .foregroundStyle(model.canSave ? Color.white : Color.codexMuted)
                .clipShape(RoundedRectangle(cornerRadius: 7))
                .disabled(!model.canSave)
            }
        }
        .padding(18)
        .background(.thinMaterial)
        .background(Color.white.opacity(0.36))
        .task {
            await model.load()
        }
    }

    private var sourceColor: Color {
        switch model.source {
        case .user:
            return Color.oarMoss
        case .env:
            return Color.oarAmber
        case .none:
            return Color.codexMuted
        }
    }

    private var protocolTitle: String {
        model.detectedProtocol ?? "未检测"
    }

    private var sourceTitle: String {
        switch model.source {
        case .user:
            return "用户配置"
        case .env:
            return "环境默认"
        case .none:
            return "未配置"
        }
    }
}

private struct AgentSettingsField<Content: View>: View {
    let title: String
    let content: Content

    init(title: String, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title)
                .font(.codexBody(11, weight: .semibold))
                .foregroundStyle(Color.codexMuted)
            content
                .font(.codexBody(12.5))
                .padding(.horizontal, 10)
                .frame(height: 32)
                .frame(maxWidth: .infinity)
                .background(Color.white.opacity(0.52))
                .clipShape(RoundedRectangle(cornerRadius: 7))
        }
    }
}

private struct AgentSettingsPill: View {
    let title: String

    var body: some View {
        Text(title)
            .font(.codexBody(11, weight: .semibold))
            .foregroundStyle(Color.codexMuted)
            .lineLimit(1)
            .padding(.horizontal, 9)
            .frame(height: 28)
            .background(Color.white.opacity(0.44))
            .clipShape(RoundedRectangle(cornerRadius: 7))
    }
}

private struct AgentSettingsMessage: View {
    let message: String
    let color: Color

    var body: some View {
        Text(message)
            .font(.codexBody(11.5, weight: .semibold))
            .foregroundStyle(color)
            .lineLimit(2)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background(Color.white.opacity(0.42))
            .clipShape(RoundedRectangle(cornerRadius: 7))
    }
}
