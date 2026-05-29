import XCTest
@testable import OAR

final class SystemLogNoiseFilterTests: XCTestCase {
    func testDropsKnownMacOSInputMethodNoise() {
        XCTAssertTrue(SystemLogNoiseFilter.isNoisySystemLogLine(
            "2026-05-29 19:04:45.072 OAR[76127:11921407] error messaging the mach port for IMKCFRunLoopWakeUpReliable"
        ))
        XCTAssertTrue(SystemLogNoiseFilter.isNoisySystemLogLine(
            "2026-05-29 19:04:45.038 OAR[76127:11921407] TSM AdjustCapsLockLEDForKeyTransitionHandling - _ISSetPhysicalKeyboardCapsLockLED Inhibit"
        ))
    }

    func testKeepsRegularApplicationErrors() {
        XCTAssertFalse(SystemLogNoiseFilter.isNoisySystemLogLine(
            "oar http facade failed to bind 127.0.0.1:8080"
        ))
        XCTAssertFalse(SystemLogNoiseFilter.isNoisySystemLogLine(
            "fatal error: database migration failed"
        ))
        XCTAssertFalse(SystemLogNoiseFilter.isNoisySystemLogLine(
            "failed while handling IMKCFRunLoopWakeUpReliable recovery path"
        ))
    }
}
