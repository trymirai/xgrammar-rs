// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "XGrammar",
    platforms: [
        .iOS("17.0"),
        .macOS("14.0"),
    ],
    products: [
        .library(name: "XGrammar", targets: ["XGrammar"]),
    ],
    targets: [
        // The UniFFI-generated XCFramework (built with `cargo swift` via platforms.toml).
        // Replace the placeholder URL/checksum with the actual release artifact.
        .binaryTarget(
            name: "xgrammar_rs",
            url: "https://placeholder.example.com/xgrammar-swift/releases/0.3.0.zip",
            checksum: "0000000000000000000000000000000000000000000000000000000000000000"
        ),
        .target(
            name: "XGrammar",
            dependencies: ["xgrammar_rs"],
            path: "bindings/swift/Sources/XGrammar"
        ),
        .testTarget(
            name: "XGrammarTests",
            dependencies: ["XGrammar"],
            path: "bindings/swift/Tests/XGrammarTests"
        ),
    ]
)
