import XCTest
@testable import OAR

final class RemoteAgentSettingsProviderTests: XCTestCase {
    override func tearDown() {
        AgentSettingsTestURLProtocol.handler = nil
        super.tearDown()
    }

    func testDetectModelsUsesBackendPreviewWithoutProviderField() async throws {
        AgentSettingsTestURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.absoluteString, "https://oar.example.test/agent/model-catalog/preview")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer oar_session_secret")

            let body = try Self.bodyData(from: request)
            let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
            XCTAssertEqual(json["base_url"] as? String, "https://api.openai.com/v1")
            XCTAssertEqual(json["api_key"] as? String, "sk-test")
            XCTAssertNil(json["provider"])
            XCTAssertNil(json["model"])

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
                      "detected_protocol": "openai-compatible",
                      "models": [
                        { "id": "gpt-4.1", "display_name": "gpt-4.1" }
                      ],
                      "recommended_model": "gpt-4.1"
                    }
                    """.utf8
                )
            )
        }

        let provider = Self.provider()
        let catalog = try await provider.detectModels(
            baseURL: "https://api.openai.com/v1",
            apiKey: "sk-test"
        )

        XCTAssertEqual(catalog.detectedProtocol, "openai-compatible")
        XCTAssertEqual(catalog.models.map(\.id), ["gpt-4.1"])
        XCTAssertEqual(catalog.recommendedModel, "gpt-4.1")
    }

    func testLoadSettingsNeverExpectsAPIKeyPlaintext() async throws {
        AgentSettingsTestURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "GET")
            XCTAssertEqual(request.url?.absoluteString, "https://oar.example.test/agent/settings")
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
                      "source": "user",
                      "detected_protocol": "anthropic",
                      "base_url": "https://api.anthropic.com/v1",
                      "selected_model": "claude-sonnet-4-5",
                      "api_key_status": "saved",
                      "can_configure": true
                    }
                    """.utf8
                )
            )
        }

        let provider = Self.provider()
        let snapshot = try await provider.loadSettings()

        XCTAssertEqual(snapshot.source, .user)
        XCTAssertEqual(snapshot.detectedProtocol, "anthropic")
        XCTAssertEqual(snapshot.selectedModel, "claude-sonnet-4-5")
        XCTAssertEqual(snapshot.apiKeyStatus, .saved)
    }

    func testSaveSettingsCanReuseSavedAPIKeyWithoutSendingPlaintext() async throws {
        AgentSettingsTestURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "PUT")
            XCTAssertEqual(request.url?.absoluteString, "https://oar.example.test/agent/settings")

            let body = try Self.bodyData(from: request)
            let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
            XCTAssertEqual(json["base_url"] as? String, "https://api.anthropic.com/v1")
            XCTAssertEqual(json["selected_model"] as? String, "claude-sonnet-4-5")
            XCTAssertNil(json["api_key"])
            XCTAssertNil(json["provider"])
            XCTAssertNil(json["model"])

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
                      "source": "user",
                      "detected_protocol": "anthropic",
                      "base_url": "https://api.anthropic.com/v1",
                      "selected_model": "claude-sonnet-4-5",
                      "api_key_status": "saved",
                      "can_configure": true
                    }
                    """.utf8
                )
            )
        }

        let provider = Self.provider()
        let snapshot = try await provider.saveSettings(
            baseURL: "https://api.anthropic.com/v1",
            apiKey: nil,
            selectedModel: "claude-sonnet-4-5"
        )

        XCTAssertEqual(snapshot.source, .user)
        XCTAssertEqual(snapshot.apiKeyStatus, .saved)
    }

    func testDetectModelsMapsRejectedAPIKeyError() async throws {
        AgentSettingsTestURLProtocol.handler = { request in
            return (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 422,
                    httpVersion: nil,
                    headerFields: nil
                )!,
                Data(#"{"error":"agent_settings_api_key_rejected"}"#.utf8)
            )
        }

        let provider = Self.provider()

        do {
            _ = try await provider.detectModels(
                baseURL: "https://www.bytecatcode.org/v1",
                apiKey: "bad-key"
            )
            XCTFail("Expected invalid API key error")
        } catch AgentSettingsProviderError.invalidAPIKey {
        } catch {
            XCTFail("Unexpected error: \(error)")
        }
    }

    private static let appSession = AppSession(
        sessionID: "oar_session_secret",
        user: AuthenticatedUser(id: "user_1", displayName: "陈敏", tenantName: "OAR 测试租户")
    )

    private static func provider() -> RemoteAgentSettingsProvider {
        RemoteAgentSettingsProvider(
            baseURL: URL(string: "https://oar.example.test")!,
            appSession: Self.appSession,
            urlSession: Self.urlSession
        )
    }

    private static var urlSession: URLSession {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [AgentSettingsTestURLProtocol.self]
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
                throw stream.streamError ?? AgentSettingsProviderError.invalidResponse
            } else {
                break
            }
        }
        return data
    }
}

private final class AgentSettingsTestURLProtocol: URLProtocol {
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
