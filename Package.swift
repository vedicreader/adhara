// swift-tools-version: 6.0
import PackageDescription

// AdharaCore.xcframework is libadhara.a (Rust, SQLite built in) with its headers; scripts/xcframework.sh builds it.
let package = Package(
    name: "Adhara",
    platforms: [.macOS(.v13), .iOS(.v16)],
    products: [.library(name: "Adhara", targets: ["Adhara"])],
    targets: [
        .binaryTarget(name: "AdharaCore", path: "swift/AdharaCore.xcframework"),
        .target(name: "Adhara", dependencies: ["AdharaCore"], path: "swift/Sources/Adhara", linkerSettings: [.linkedLibrary("c++")]),
        .testTarget(name: "AdharaTests", dependencies: ["Adhara"], path: "swift/Tests/AdharaTests"),
    ]
)
