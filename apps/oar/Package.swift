// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "OAR",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .executable(name: "OAR", targets: ["OAR"])
    ],
    targets: [
        .executableTarget(
            name: "OAR",
            path: "Sources/OAR"
        ),
        .testTarget(
            name: "OARTests",
            dependencies: ["OAR"],
            path: "Tests/OARTests"
        )
    ]
)
