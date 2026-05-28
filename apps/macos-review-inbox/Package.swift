// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "OARReviewInbox",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .executable(name: "OARReviewInbox", targets: ["OARReviewInbox"])
    ],
    targets: [
        .executableTarget(
            name: "OARReviewInbox",
            path: "Sources/OARReviewInbox"
        ),
        .testTarget(
            name: "OARReviewInboxTests",
            dependencies: ["OARReviewInbox"],
            path: "Tests/OARReviewInboxTests"
        )
    ]
)
