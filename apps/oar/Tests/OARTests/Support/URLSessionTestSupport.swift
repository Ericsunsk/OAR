import Foundation
import XCTest

enum URLRequestBodyTestSupport {
    static func bodyData(
        from request: URLRequest,
        file: StaticString = #filePath,
        line: UInt = #line
    ) throws -> Data {
        if let httpBody = request.httpBody {
            return httpBody
        }

        let stream = try XCTUnwrap(request.httpBodyStream, file: file, line: line)
        stream.open()
        defer {
            stream.close()
        }

        var data = Data()
        let bufferSize = 1_024
        let buffer = UnsafeMutablePointer<UInt8>.allocate(capacity: bufferSize)
        defer {
            buffer.deallocate()
        }

        while stream.hasBytesAvailable {
            let bytesRead = stream.read(buffer, maxLength: bufferSize)
            if bytesRead > 0 {
                data.append(buffer, count: bytesRead)
            } else if bytesRead < 0 {
                throw stream.streamError ?? URLError(.cannotDecodeRawData)
            } else {
                break
            }
        }

        return data
    }
}

class HTTPURLProtocolStub: URLProtocol {
    typealias Handler = (URLRequest) throws -> (HTTPURLResponse, Data)

    private static var handlers: [ObjectIdentifier: Handler] = [:]

    class var handler: Handler? {
        get {
            handlers[ObjectIdentifier(self)]
        }
        set {
            if let newValue {
                handlers[ObjectIdentifier(self)] = newValue
            } else {
                handlers.removeValue(forKey: ObjectIdentifier(self))
            }
        }
    }

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
