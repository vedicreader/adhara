// swift-tools-version: 6.0
import Foundation
import PackageDescription

// AdharaCore is libadhara.a (Rust, SQLite built in) with its headers. A checkout that ran scripts/xcframework.sh
// links that build. A package that depends on this repository downloads the release build; scripts/swift_release.sh
// writes its url and checksum.
let local = "swift/AdharaCore.xcframework"
let core: Target = FileManager.default.fileExists(atPath: "\(Context.packageDirectory)/\(local)")
    ? .binaryTarget(name: "AdharaCore", path: local)
    : .binaryTarget(name: "AdharaCore",
                    url: "https://github.com/vedicreader/adhara/releases/download/v0.1.0/AdharaCore.xcframework.zip",
                    checksum: "0000000000000000000000000000000000000000000000000000000000000000")

let package = Package(
    name: "Adhara",
    platforms: [.macOS(.v13)],
    products: [.library(name: "Adhara", targets: ["Adhara"])],
    targets: [
        core,
        .target(name: "Adhara", dependencies: ["AdharaCore"], path: "swift/Sources/Adhara", linkerSettings: [.linkedLibrary("c++")]),
        .testTarget(name: "AdharaTests", dependencies: ["Adhara"], path: "swift/Tests/AdharaTests"),
    ]
)
