// swift-tools-version: 6.2

import PackageDescription

let package = Package(
    name: "MacHuntSwift",
    platforms: [.macOS(.v14)],
    products: [
        .executable(name: "MacHunt", targets: ["MacHuntApp"]),
        .executable(name: "MacHuntCommand", targets: ["MacHuntCLI"]),
        .library(name: "MacHuntCore", targets: ["MacHuntCore"]),
    ],
    targets: [
        .target(
            name: "MacHuntCore",
            linkerSettings: [.linkedLibrary("sqlite3")]
        ),
        .executableTarget(
            name: "MacHuntApp",
            dependencies: ["MacHuntCore"],
            linkerSettings: [
                .linkedFramework("AppKit"),
                .linkedFramework("Carbon"),
                .linkedFramework("CoreServices"),
                .linkedFramework("QuickLookUI"),
                .linkedFramework("ServiceManagement"),
            ]
        ),
        .executableTarget(
            name: "MacHuntCLI",
            dependencies: ["MacHuntCore"]
        ),
        .testTarget(
            name: "MacHuntCoreTests",
            dependencies: ["MacHuntCore"]
        ),
    ],
    swiftLanguageModes: [.v6]
)
