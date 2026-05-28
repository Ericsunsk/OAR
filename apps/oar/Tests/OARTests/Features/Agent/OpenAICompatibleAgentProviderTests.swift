import XCTest
@testable import OAR

final class OpenAICompatibleAgentProviderTests: XCTestCase {
    override func tearDown() {
        AgentTestURLProtocol.handler = nil
        super.tearDown()
    }

    func testSendUsesOpenAICompatibleRequestShape() async throws {
        AgentTestURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.absoluteString, "https://llm.example.test/v1/chat/completions")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer sk-secret")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")

            let body = try Self.bodyData(from: request)
            let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
            XCTAssertEqual(json["model"] as? String, "model-test")
            XCTAssertFalse(String(data: body, encoding: .utf8)?.contains("sk-secret") ?? true)

            let messages = try XCTUnwrap(json["messages"] as? [[String: Any]])
            XCTAssertEqual(messages.first?["role"] as? String, "system")
            XCTAssertEqual(messages.last?["role"] as? String, "user")
            XCTAssertEqual(messages.last?["content"] as? String, "解释风险")

            return (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!,
                Data(
                    """
                    {
                      "choices": [
                        { "message": { "role": "assistant", "content": "风险来自连续延期。" } }
                      ]
                    }
                    """.utf8
                )
            )
        }

        let provider = OpenAICompatibleAgentProvider(urlSession: Self.urlSession)
        let reply = try await provider.send(
            messages: [AgentMessage(role: .user, text: "解释风险")],
            context: AgentConversationContext(
                title: "KR 风险",
                riskReason: "连续延期",
                actionSummary: "更新进度",
                evidenceSummaries: ["连续两周延期"]
            ),
            settings: ResolvedAgentSettings(
                baseURL: URL(string: "https://llm.example.test/v1")!,
                model: "model-test",
                apiKey: "sk-secret"
            )
        )

        XCTAssertEqual(reply.role, .assistant)
        XCTAssertEqual(reply.text, "风险来自连续延期。")
    }

    func testUnauthorizedStatusMapsWithoutLeakingAPIKey() async {
        AgentTestURLProtocol.handler = { request in
            (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 401,
                    httpVersion: nil,
                    headerFields: nil
                )!,
                Data("bad key".utf8)
            )
        }

        let provider = OpenAICompatibleAgentProvider(urlSession: Self.urlSession)

        do {
            _ = try await provider.send(
                messages: [AgentMessage(role: .user, text: "hi")],
                context: .empty,
                settings: ResolvedAgentSettings(
                    baseURL: URL(string: "https://llm.example.test/v1")!,
                    model: "model-test",
                    apiKey: "sk-secret"
                )
            )
            XCTFail("Expected unauthorized error")
        } catch let error as AgentProviderError {
            XCTAssertEqual(error, .unauthorized)
            XCTAssertFalse(error.localizedDescription.contains("sk-secret"))
        } catch {
            XCTFail("Unexpected error: \(error)")
        }
    }

    private static var urlSession: URLSession {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [AgentTestURLProtocol.self]
        return URLSession(configuration: configuration)
    }

    private static func bodyData(from request: URLRequest) throws -> Data {
        if let httpBody = request.httpBody {
            return httpBody
        }

        let stream = try XCTUnwrap(request.httpBodyStream)
        stream.open()
        defer { stream.close() }

        var data = Data()
        let bufferSize = 1_024
        let buffer = UnsafeMutablePointer<UInt8>.allocate(capacity: bufferSize)
        defer { buffer.deallocate() }

        while stream.hasBytesAvailable {
            let bytesRead = stream.read(buffer, maxLength: bufferSize)
            if bytesRead > 0 {
                data.append(buffer, count: bytesRead)
            } else if bytesRead < 0 {
                throw stream.streamError ?? AgentProviderError.invalidResponse
            } else {
                break
            }
        }
        return data
    }
}

private final class AgentTestURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data))?

    override class func canInit(with request: URLRequest) -> Bool {
        true
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        do {
            let handler = try XCTUnwrap(Self.handler)
            let (response, data) = try handler(request)
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: data)
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {
    }
}
