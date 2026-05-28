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
