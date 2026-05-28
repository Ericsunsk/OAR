import Foundation

struct CreateFeishuQRCodeSessionResponseDTO: Codable, Equatable {
    let sessionID: String
    let qrPageURL: URL
    let expiresAt: String

    enum CodingKeys: String, CodingKey {
        case sessionID = "session_id"
        case qrPageURL = "qr_page_url"
        case expiresAt = "expires_at"
    }
}

struct FeishuQRCodeSessionStatusResponseDTO: Codable, Equatable {
    let status: FeishuQRCodeSessionStatusDTO
    let qrSession: CreateFeishuQRCodeSessionResponseDTO?
    let oarSession: OARSessionDTO?
    let user: AuthenticatedUserDTO?
    let safeMessage: String?

    enum CodingKeys: String, CodingKey {
        case status
        case qrSession = "qr_session"
        case oarSession = "oar_session"
        case user
        case safeMessage = "safe_message"
    }
}

struct AuthLoginEventDTO: Codable, Equatable {
    let event: AuthLoginEventKindDTO
    let sessionID: String
    let qrSession: CreateFeishuQRCodeSessionResponseDTO?
    let oarSession: OARSessionDTO?
    let user: AuthenticatedUserDTO?
    let safeMessage: String?
    let eventID: String?

    enum CodingKeys: String, CodingKey {
        case event
        case sessionID = "session_id"
        case qrSession = "qr_session"
        case oarSession = "oar_session"
        case user
        case safeMessage = "safe_message"
        case eventID = "event_id"
    }
}

enum FeishuQRCodeSessionStatusDTO: String, Codable {
    case pending
    case authorized
    case denied
    case expired
}

enum AuthLoginEventKindDTO: String, Codable {
    case pending
    case authorized
    case denied
    case expired
    case keepalive
}

struct OARSessionDTO: Codable, Equatable {
    let sessionID: String

    enum CodingKeys: String, CodingKey {
        case sessionID = "session_id"
    }
}

struct AuthenticatedUserDTO: Codable, Equatable {
    let id: String
    let displayName: String
    let tenantName: String

    enum CodingKeys: String, CodingKey {
        case id
        case displayName = "display_name"
        case tenantName = "tenant_name"
    }
}

extension CreateFeishuQRCodeSessionResponseDTO {
    func toDomain(dateParser: ISO8601DateFormatter = ISO8601DateFormatter()) throws -> FeishuQRCodeAuthSession {
        guard let expiresAtDate = dateParser.date(from: expiresAt) else {
            throw AuthProviderError.invalidResponse
        }

        return FeishuQRCodeAuthSession(
            id: sessionID,
            qrPageURL: qrPageURL,
            expiresAt: expiresAtDate
        )
    }
}

extension FeishuQRCodeSessionStatusResponseDTO {
    func toDomainState(dateParser: ISO8601DateFormatter = ISO8601DateFormatter()) throws -> AuthSessionState {
        switch status {
        case .pending:
            guard let qrSession else {
                throw AuthProviderError.invalidResponse
            }
            return .waitingForScan(try qrSession.toDomain(dateParser: dateParser))
        case .authorized:
            guard let oarSession, let user else {
                throw AuthProviderError.invalidResponse
            }
            return .authorized(
                AppSession(
                    sessionID: oarSession.sessionID,
                    user: AuthenticatedUser(
                        id: user.id,
                        displayName: user.displayName,
                        tenantName: user.tenantName
                    )
                )
            )
        case .denied:
            return .denied(safeMessage ?? "飞书扫码授权已取消。")
        case .expired:
            return .expired
        }
    }
}

extension AuthLoginEventDTO {
    func toDomainEvent(dateParser: ISO8601DateFormatter = ISO8601DateFormatter()) throws -> AuthLoginEvent {
        switch event {
        case .pending:
            guard let qrSession else {
                throw AuthProviderError.invalidResponse
            }
            return .pending(
                sessionID: sessionID,
                qrSession: try qrSession.toDomain(dateParser: dateParser)
            )
        case .authorized:
            guard let oarSession, let user else {
                throw AuthProviderError.invalidResponse
            }
            return .authorized(
                sessionID: sessionID,
                appSession: AppSession(
                    sessionID: oarSession.sessionID,
                    user: AuthenticatedUser(
                        id: user.id,
                        displayName: user.displayName,
                        tenantName: user.tenantName
                    )
                )
            )
        case .denied:
            return .denied(sessionID: sessionID, message: safeMessage ?? "飞书扫码授权已取消。")
        case .expired:
            return .expired(sessionID: sessionID)
        case .keepalive:
            return .keepalive(sessionID: sessionID)
        }
    }
}
