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
        .executable(name: "examples", targets: ["Examples"]),
    ],
    dependencies: [
        .package(url: "https://github.com/apple/swift-argument-parser", from: "1.3.0"),
    ],
    targets: [
        .binaryTarget(
            name: "xgrammar_rsFFI",
            path: "bindings/swift/xgrammar_rs.xcframework"
        ),
        .target(
            name: "XGrammar",
            dependencies: ["xgrammar_rsFFI"],
            path: "bindings/swift/Sources/XGrammar"
        ),
        .executableTarget(
            name: "Examples",
            dependencies: [
                "XGrammar",
                .product(name: "ArgumentParser", package: "swift-argument-parser"),
            ],
            path: "bindings/swift/Sources/Examples",
            exclude: ["README.md"]
        ),
        .testTarget(
            name: "XGrammarTests",
            dependencies: ["XGrammar"],
            path: "bindings/swift/Tests/XGrammarTests"
        ),
    ]
)
