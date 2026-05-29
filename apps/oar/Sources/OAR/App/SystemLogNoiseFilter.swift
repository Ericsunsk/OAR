import Darwin
import Foundation

enum SystemLogNoiseFilter {
    private static let queue = DispatchQueue(label: "org.oar.stderr-noise-filter")
    private static let installLock = NSLock()
    private static var installed = false

    static func install() {
        installLock.lock()
        defer { installLock.unlock() }

        guard !installed else { return }

        var fds = [Int32](repeating: 0, count: 2)
        guard pipe(&fds) == 0 else { return }

        let originalStderr = dup(STDERR_FILENO)
        guard originalStderr >= 0 else {
            close(fds[0])
            close(fds[1])
            return
        }

        guard dup2(fds[1], STDERR_FILENO) >= 0 else {
            close(originalStderr)
            close(fds[0])
            close(fds[1])
            return
        }

        close(fds[1])
        installed = true

        queue.async {
            readAndForward(readFD: fds[0], writeFD: originalStderr)
        }
    }

    static func isNoisySystemLogLine(_ line: String) -> Bool {
        line.contains("error messaging the mach port for IMKCFRunLoopWakeUpReliable")
            || (
                line.contains("TSM AdjustCapsLockLEDForKeyTransitionHandling")
                    && line.contains("_ISSetPhysicalKeyboardCapsLockLED Inhibit")
            )
    }

    private static func readAndForward(readFD: Int32, writeFD: Int32) {
        let handle = FileHandle(fileDescriptor: readFD, closeOnDealloc: true)
        var pending = Data()

        while true {
            let chunk = try? handle.read(upToCount: 4096)
            guard let chunk, !chunk.isEmpty else { break }

            pending.append(chunk)
            while let newline = pending.firstIndex(of: 0x0A) {
                let line = pending.prefix(through: newline)
                pending.removeSubrange(...newline)
                forwardUnlessNoisy(Data(line), to: writeFD)
            }
        }

        if !pending.isEmpty {
            forwardUnlessNoisy(pending, to: writeFD)
        }
        close(writeFD)
    }

    private static func forwardUnlessNoisy(_ data: Data, to fd: Int32) {
        let line = String(data: data, encoding: .utf8) ?? ""
        guard !isNoisySystemLogLine(line) else { return }
        writeAll(data, to: fd)
    }

    private static func writeAll(_ data: Data, to fd: Int32) {
        data.withUnsafeBytes { rawBuffer in
            guard let base = rawBuffer.bindMemory(to: UInt8.self).baseAddress else { return }

            var offset = 0
            while offset < data.count {
                let written = write(fd, base.advanced(by: offset), data.count - offset)
                if written <= 0 {
                    return
                }
                offset += written
            }
        }
    }
}
