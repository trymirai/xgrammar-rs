import ArgumentParser
import Foundation

@main
struct Example: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "examples",
        abstract: "XGrammar examples"
    )

    @Argument(help: "Mode: json-grammar", transform: { $0.lowercased() })
    var mode: String = "json-grammar"

    mutating func run() throws {
        switch mode {
        case "json-grammar":
            try runJsonGrammar()
        default:
            throw ValidationError("Unknown mode: \(mode)")
        }
    }
}
