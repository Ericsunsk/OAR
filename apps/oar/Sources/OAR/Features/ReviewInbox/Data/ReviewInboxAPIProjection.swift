extension ReviewInboxAPISnapshot {
    func toDisplaySnapshot() -> ReviewInboxDisplaySnapshot {
        ReviewInboxDisplaySnapshot(
            items: items.map(\.displayModel),
            evidence: evidence.map(\.displayModel),
            actions: proposedActions.map(\.displayModel),
            ledgerEvents: ledgerEvents.map(\.displayModel)
        )
    }
}

private extension ReviewInboxItemDTO {
    var displayModel: ReviewInboxDisplayItem {
        ReviewInboxDisplayItem(
            id: id,
            objectiveTitle: objectiveTitle,
            keyResultTitle: keyResultTitle,
            ownerName: ownerDisplayName,
            weekLabel: weekLabel,
            riskLevel: riskScore.riskLevel,
            riskReason: riskReason,
            confidenceScore: confidenceScore,
            status: status.displayStatus,
            lastUpdatedAt: updatedAtDisplay,
            syncCursor: syncCursor
        )
    }
}

private extension ProposedActionDTO {
    var displayModel: ReviewInboxSuggestedAction {
        ReviewInboxSuggestedAction(
            id: id,
            reviewItemId: reviewItemID,
            version: version,
            actionType: kind.displayActionType,
            rationale: rationale,
            expectedImpact: expectedImpact,
            dryRunResultSummary: dryRunResultSummary,
            estimatedWriteTargetsCount: estimatedWriteTargetsCount,
            gateState: decision.displayGateState(status: status)
        )
    }
}

private extension EvidenceItemDTO {
    var displayModel: ReviewInboxDisplayEvidence {
        ReviewInboxDisplayEvidence(
            id: id,
            reviewItemId: reviewItemID,
            sourceType: sourceKind.displaySource,
            sourceRef: locator ?? sourceID,
            capturedAt: observedAtDisplay,
            summary: summary,
            signalType: signalType.displaySignal,
            trustScore: trustScore
        )
    }
}

private extension LedgerEventDTO {
    var displayModel: ReviewInboxTimelineEvent {
        ReviewInboxTimelineEvent(
            id: id,
            actionId: actionID,
            stage: stage.displayStage,
            stageStatus: stageStatus.displayStatus,
            timestamp: timestampDisplay,
            message: message,
            idempotencyKey: idempotencyKey
        )
    }
}

private extension UInt32 {
    var riskLevel: ReviewInboxRiskLevel {
        switch self {
        case 90...:
            return .critical
        case 70..<90:
            return .high
        case 40..<70:
            return .medium
        default:
            return .low
        }
    }
}

private extension ReviewInboxItemStatusDTO {
    var displayStatus: ReviewInboxDisplayStatus {
        switch self {
        case .open:
            return .needsConfirmation
        case .confirmed, .executing:
            return .confirmed
        case .succeeded:
            return .executed
        case .failed:
            return .failed
        case .rejected, .withdrawn:
            return .rejected
        }
    }
}

private extension ProposedActionKindDTO {
    var displayActionType: ReviewInboxActionType {
        switch self {
        case .createKrProgress, .updateKrProgress:
            return .updateProgress
        case .pingOwner:
            return .pingOwner
        case .createTask:
            return .createTask
        case .scheduleReview:
            return .scheduleReview
        case .deleteKrProgressDryRun, .custom:
            return .pingOwner
        }
    }
}

private extension Optional where Wrapped == ProposedActionDecisionDTO {
    func displayGateState(status: ProposedActionStatusDTO) -> ReviewInboxGateState {
        switch self {
        case .some(.confirm), .some(.editThenConfirm):
            return .approved
        case .some(.reject):
            return .rejected
        case .none:
            switch status {
            case .published:
                return .pending
            case .draft:
                return .draft
            case .superseded:
                return .superseded
            case .withdrawn:
                return .withdrawn
            }
        }
    }
}

private extension EvidenceSourceKindDTO {
    var displaySource: ReviewInboxEvidenceSource {
        switch self {
        case .okrProgress:
            return .okr
        case .larkMinutes:
            return .meeting
        case .larkDoc:
            return .doc
        case .larkTask:
            return .task
        case .larkCalendar:
            return .calendar
        case .larkIM:
            return .im
        case .manualReviewNote, .auditEvent:
            return .doc
        }
    }
}

private extension SignalTypeDTO {
    var displaySignal: ReviewInboxSignal {
        switch self {
        case .progress:
            return .progress
        case .blocker:
            return .blocker
        case .dependency:
            return .dependency
        case .cadence:
            return .cadence
        }
    }
}

private extension LedgerStageDTO {
    var displayStage: ReviewInboxTimelineStage {
        switch self {
        case .confirmedAction:
            return .confirmedAction
        case .operationLedger:
            return .operationLedger
        case .platformAdapter:
            return .larkAdapter
        case .auditEvent:
            return .auditEvent
        }
    }
}

private extension LedgerStatusDTO {
    var displayStatus: ReviewInboxTimelineStatus {
        switch self {
        case .pending:
            return .pending
        case .ok:
            return .ok
        case .error:
            return .error
        }
    }
}
