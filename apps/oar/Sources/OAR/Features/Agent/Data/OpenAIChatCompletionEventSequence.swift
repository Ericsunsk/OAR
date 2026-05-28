import Foundation

struct OpenAIChatCompletionEventSequence<Base: AsyncSequence>: AsyncSequence where Base.Element == ServerSentEvent {
    typealias Element = AgentStreamEvent

    let events: Base
    let decoder: JSONDecoder

    func makeAsyncIterator() -> Iterator {
        Iterator(eventIterator: events.makeAsyncIterator(), decoder: decoder)
    }

    struct Iterator: AsyncIteratorProtocol {
        var eventIterator: Base.AsyncIterator
        let decoder: JSONDecoder

        mutating func next() async throws -> AgentStreamEvent? {
            while let event = try await eventIterator.next() {
                guard let streamEvent = try streamEvent(from: event) else { continue }
                return streamEvent
            }
            return nil
        }

        private func streamEvent(from event: ServerSentEvent) throws -> AgentStreamEvent? {
            let payload = event.data.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !payload.isEmpty else { return nil }
            guard payload != "[DONE]" else { return .completed }

            let dto = try decoder.decode(OpenAIChatCompletionStreamChunkDTO.self, from: Data(payload.utf8))
            let content = dto.choices.compactMap(\.delta.content).joined()
            guard !content.isEmpty else { return nil }
            return .delta(content)
        }
    }
}

private struct OpenAIChatCompletionStreamChunkDTO: Decodable {
    let choices: [Choice]

    struct Choice: Decodable {
        let delta: Delta
    }

    struct Delta: Decodable {
        let content: String?
    }
}
