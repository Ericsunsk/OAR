import AppKit
import SwiftUI

struct ChatInputBar: View {
    @Binding var draft: String
    let isSending: Bool
    let isEnabled: Bool
    let send: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            ZStack(alignment: .topLeading) {
                AgentComposerTextView(text: $draft, submit: send)
                    .frame(maxWidth: .infinity, minHeight: 32, maxHeight: 64)
                    .disabled(!isEnabled)

                if draft.isEmpty {
                    Text("问计划、风险、证据或动作")
                        .font(.codexBody(13))
                        .foregroundStyle(Color.codexMuted.opacity(0.72))
                        .padding(.top, 7)
                        .allowsHitTesting(false)
                }
            }

            Button(action: send) {
                Image(systemName: isSending ? "hourglass" : "arrow.up")
                    .font(.system(size: 11, weight: .bold))
                    .frame(width: 25, height: 25)
                    .background(sendDisabled ? Color.codexMuted.opacity(0.14) : Color.codexInk)
                    .foregroundStyle(sendDisabled ? Color.codexMuted : Color.white)
                    .clipShape(Circle())
            }
            .buttonStyle(.plain)
            .disabled(sendDisabled)
            .accessibilityLabel("发送消息")
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .frame(minHeight: 46)
        .background(Color.white.opacity(0.42))
    }

    private var sendDisabled: Bool {
        !isEnabled || isSending || draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }
}

private struct AgentComposerTextView: NSViewRepresentable {
    @Binding var text: String
    let submit: () -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(text: $text, submit: submit)
    }

    func makeNSView(context: Context) -> NSScrollView {
        let textStorage = NSTextStorage()
        let layoutManager = NSLayoutManager()
        textStorage.addLayoutManager(layoutManager)
        let textContainer = NSTextContainer(containerSize: NSSize(width: 0, height: CGFloat.greatestFiniteMagnitude))
        textContainer.widthTracksTextView = true
        textContainer.lineFragmentPadding = 0
        layoutManager.addTextContainer(textContainer)

        let textView = EditableTextView(frame: NSRect(x: 0, y: 0, width: 240, height: 32), textContainer: textContainer)
        textView.delegate = context.coordinator
        textView.drawsBackground = false
        textView.isEditable = true
        textView.isSelectable = true
        textView.isRichText = false
        textView.isAutomaticQuoteSubstitutionEnabled = false
        textView.isAutomaticDashSubstitutionEnabled = false
        textView.font = .systemFont(ofSize: 13)
        textView.textColor = .labelColor
        textView.insertionPointColor = .labelColor
        textView.textContainerInset = NSSize(width: 0, height: 6)
        textView.minSize = NSSize(width: 0, height: 32)
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = false
        textView.autoresizingMask = [.width]

        let scrollView = NSScrollView()
        scrollView.borderType = .noBorder
        scrollView.drawsBackground = false
        scrollView.hasVerticalScroller = false
        scrollView.hasHorizontalScroller = false
        scrollView.autoresizesSubviews = true
        scrollView.documentView = textView
        return scrollView
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        context.coordinator.text = $text
        context.coordinator.submit = submit

        guard let textView = scrollView.documentView as? NSTextView else { return }

        // Keep the text view width in sync with the clip view so the
        // full area is clickable / editable.
        let clipWidth = scrollView.contentSize.width
        if clipWidth > 0, abs(textView.frame.width - clipWidth) > 0.5 {
            textView.setFrameSize(NSSize(width: clipWidth, height: textView.frame.height))
        }

        // Only sync text when it was changed externally (e.g. cleared after
        // send). Preserve the insertion point so the cursor doesn't jump.
        if textView.string != text {
            let selectedRanges = textView.selectedRanges
            textView.string = text
            let textLength = (textView.string as NSString).length
            let clampedRanges = selectedRanges.compactMap { value -> NSValue? in
                let range = value.rangeValue
                guard range.location != NSNotFound else { return nil }
                let location = min(range.location, textLength)
                let upperBound = min(NSMaxRange(range), textLength)
                return NSValue(range: NSRange(location: location, length: max(0, upperBound - location)))
            }
            textView.selectedRanges = clampedRanges.isEmpty
                ? [NSValue(range: NSRange(location: textLength, length: 0))]
                : clampedRanges
        }
    }

    /// Subclass that guarantees first-responder acceptance for keyboard input.
    private final class EditableTextView: NSTextView {
        override var acceptsFirstResponder: Bool { true }

        override func becomeFirstResponder() -> Bool {
            let result = super.becomeFirstResponder()
            insertionPointColor = .labelColor
            return result
        }
    }

    final class Coordinator: NSObject, NSTextViewDelegate {
        var text: Binding<String>
        var submit: () -> Void

        init(text: Binding<String>, submit: @escaping () -> Void) {
            self.text = text
            self.submit = submit
        }

        func textDidChange(_ notification: Notification) {
            guard let textView = notification.object as? NSTextView else { return }
            text.wrappedValue = textView.string
        }

        func textView(_ textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            if commandSelector == #selector(NSResponder.insertNewline(_:)) {
                submit()
                return true
            }
            return false
        }
    }
}
