import XCTest

@testable import XGrammar

final class XGrammarTests: XCTestCase {
    func testRegexGrammarRoundtrip() throws {
        let grammar = try grammarFromRegex(regexString: "a+", printConvertedEbnf: false)
        let rendered = grammar.toString()
        XCTAssertFalse(rendered.isEmpty)
    }

    func testBuiltinJsonGrammar() {
        let grammar = grammarBuiltinJsonGrammar()
        XCTAssertFalse(grammar.toString().isEmpty)
    }
}
