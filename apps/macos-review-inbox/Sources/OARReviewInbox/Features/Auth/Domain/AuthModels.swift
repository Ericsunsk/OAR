import Foundation

struct AuthenticatedUser: Equatable {
    let id: String
    let displayName: String
    let tenantName: String
}

struct AppSession: Equatable {
    let sessionID: String
    let user: AuthenticatedUser
}

struct FeishuQRCodeAuthSession: Equatable {
    let id: String
    let qrPageURL: URL
    let expiresAt: Date
}

enum AuthSessionState: Equatable {
    case signedOut
    case waitingForScan(FeishuQRCodeAuthSession)
    case authorized(AppSession)
    case denied(String)
    case expired
}

enum AuthTransportState: Equatable {
    case idle
    case sseConnecting
    case sseLive
    case pollingFallback
}

enum AuthLoginEvent: Equatable {
    case pending(sessionID: String, qrSession: FeishuQRCodeAuthSession)
    case authorized(sessionID: String, appSession: AppSession)
    case denied(sessionID: String, message: String)
    case expired(sessionID: String)
    case keepalive(sessionID: String)

    var sessionID: String {
        switch self {
        case let .pending(sessionID, _),
             let .authorized(sessionID, _),
             let .denied(sessionID, _),
             let .expired(sessionID),
             let .keepalive(sessionID):
            return sessionID
        }
    }
}
