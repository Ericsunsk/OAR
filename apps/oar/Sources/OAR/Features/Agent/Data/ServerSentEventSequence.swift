import Foundation

struct ServerSentEvent {
    let data: String
}

struct ServerSentEventSequence<Base: AsyncSequence>: AsyncSequence where Base.Element == UInt8 {
    typealias Element = ServerSentEvent

    let bytes: Base

    func makeAsyncIterator() -> Iterator {
        Iterator(byteIterator: bytes.makeAsyncIterator())
    }

    struct Iterator: AsyncIteratorProtocol {
        var byteIterator: Base.AsyncIterator
        private var parser = ServerSentEventParser()
        private var lineBuffer: [UInt8] = []
        private var pendingEvents: [ServerSentEvent] = []

        init(byteIterator: Base.AsyncIterator) {
            self.byteIterator = byteIterator
        }

        mutating func next() async throws -> ServerSentEvent? {
            while pendingEvents.isEmpty {
                guard let byte = try await byteIterator.next() else {
                    if !lineBuffer.isEmpty {
                        pendingEvents.append(contentsOf: parser.feed(Self.line(from: lineBuffer)))
                        lineBuffer.removeAll()
                    }
                    pendingEvents.append(contentsOf: parser.finish())
                    break
                }

                if byte == 10 {
                    pendingEvents.append(contentsOf: parser.feed(Self.line(from: lineBuffer)))
                    lineBuffer.removeAll(keepingCapacity: true)
                } else {
                    lineBuffer.append(byte)
                }
            }

            guard !pendingEvents.isEmpty else { return nil }
            return pendingEvents.removeFirst()
        }

        private static func line(from bytes: [UInt8]) -> String {
            var line = String(decoding: bytes, as: UTF8.self)
            if line.last == "\r" {
                line.removeLast()
            }
            return line
        }
    }
}

private struct ServerSentEventParser {
    private var dataLines: [String] = []

    mutating func feed(_ line: String) -> [ServerSentEvent] {
        if line.trimmingCharacters(in: .whitespaces).isEmpty {
            return dispatch()
        }

        guard !line.hasPrefix(":") else { return [] }

        let fieldAndValue = parseField(line)
        guard fieldAndValue.field == "data" else { return [] }
        dataLines.append(fieldAndValue.value)
        return []
    }

    mutating func finish() -> [ServerSentEvent] {
        dispatch()
    }

    private mutating func dispatch() -> [ServerSentEvent] {
        guard !dataLines.isEmpty else { return [] }
        let event = ServerSentEvent(data: dataLines.joined(separator: "\n"))
        dataLines.removeAll()
        return [event]
    }

    private func parseField(_ line: String) -> (field: String, value: String) {
        guard let separatorIndex = line.firstIndex(of: ":") else {
            return (line, "")
        }

        let field = String(line[..<separatorIndex])
        var value = String(line[line.index(after: separatorIndex)...])
        if value.first == " " {
            value.removeFirst()
        }
        return (field, value)
    }
}
