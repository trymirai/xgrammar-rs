import XCTest

@testable import XGrammar

final class XGrammarTests: XCTestCase {
    // 40 tokens: token 31 is the sign bit of the first mask word, tokens 32... fill a second word.
    private static let vocab = (0..<39).map { "t\($0)" } + ["</s>"]
    private static let stopToken: Int32 = 39

    private func tokenizerInfo() throws -> TokenizerInfo {
        try TokenizerInfo(
            encodedVocab: Self.vocab, vocabType: 0, vocabSize: nil, stopTokenIds: [Self.stopToken], addPrefixSpace: false)
    }

    private func compiler() throws -> GrammarCompiler {
        GrammarCompiler(tokenizerInfo: try tokenizerInfo(), maxThreads: 1, cacheEnabled: false, cacheLimitBytes: -1)
    }

    private func matcher(_ compiled: CompiledGrammar, terminateWithoutStopToken: Bool = false) throws -> GrammarMatcher {
        try GrammarMatcher(
            compiledGrammar: compiled, overrideStopTokens: nil, terminateWithoutStopToken: terminateWithoutStopToken,
            maxRollbackTokens: -1, defaultTemperature: nil)
    }

    private func allowed(_ mask: TokenBitmask) -> [Int32] {
        (0..<Int32(Self.vocab.count)).filter { (UInt32(bitPattern: mask.words[Int($0 / 32)]) >> UInt32($0 % 32)) & 1 == 1 }
    }

    func testRegexGrammarRoundtrip() throws {
        let grammar = try grammarFromRegex(regexString: "a+", printConvertedEbnf: false)
        let rendered = grammar.toString()
        XCTAssertFalse(rendered.isEmpty)
    }

    func testBuiltinJsonGrammar() {
        let grammar = grammarBuiltinJsonGrammar()
        XCTAssertFalse(grammar.toString().isEmpty)
    }

    func testOwnedMaskPacksSignBit() throws {
        let matcher = try matcher(try compiler().compileGrammarFromStrings(ebnfString: "root ::= \"t31\"", rootRuleName: "root"))
        let mask = try matcher.nextTokenBitmask()
        XCTAssertEqual(mask.words.count, 2)
        XCTAssertTrue(mask.needApply)
        XCTAssertLessThan(mask.words[0], 0)
        XCTAssertEqual(allowed(mask), [3, 31])
    }

    func testDecodeLoopAcceptRejectStop() throws {
        let matcher = try matcher(try compiler().compileGrammarFromStrings(ebnfString: "root ::= \"t1\" \"t2\"", rootRuleName: "root"))
        XCTAssertFalse(matcher.acceptToken(tokenId: 3, debugPrint: false))
        XCTAssertTrue(matcher.acceptToken(tokenId: 1, debugPrint: false))
        XCTAssertEqual(allowed(try matcher.nextTokenBitmask()), [2])
        XCTAssertTrue(matcher.acceptToken(tokenId: 2, debugPrint: false))
        XCTAssertEqual(allowed(try matcher.nextTokenBitmask()), [Self.stopToken])
        XCTAssertTrue(matcher.acceptStopToken())
        XCTAssertTrue(matcher.isTerminated())
        XCTAssertThrowsError(try matcher.nextTokenBitmask())
    }

    func testRollbackForkResetAndJumpForward() throws {
        let matcher = try matcher(try compiler().compileGrammarFromStrings(ebnfString: "root ::= \"t1\" \"t2t3\"", rootRuleName: "root"))
        XCTAssertEqual(try matcher.findJumpForwardString(), "t1t2t3")
        XCTAssertTrue(matcher.acceptToken(tokenId: 1, debugPrint: false))
        XCTAssertEqual(try matcher.findJumpForwardBytes(), Data("t2t3".utf8))
        let fork = matcher.fork()
        try matcher.rollback(numTokens: 1)
        XCTAssertEqual(allowed(try matcher.nextTokenBitmask()), [1])
        XCTAssertEqual(allowed(try fork.nextTokenBitmask()), [2])
        fork.reset()
        XCTAssertEqual(try fork.nextTokenBitmask(), try matcher.nextTokenBitmask())
        XCTAssertEqual(matcher.tokenizerInfo().vocabSize(), Int32(Self.vocab.count))
    }

    func testLarkNamedGrammarsAndCaptures() throws {
        let named = NamedGrammars()
        named.addGrammar(name: "first", grammar: try grammarFromEbnf(ebnfString: "root ::= \"t1\"", rootRuleName: "root"))
        named.addLark(name: "second", source: "start: v\nv[capture]: \"t\" /[0-9]/")
        let compiled = try compiler().compileLark(larkString: "start: @first @second", namedGrammars: named)
        let matcher = try matcher(compiled, terminateWithoutStopToken: true)
        XCTAssertTrue(matcher.acceptString(input: "t1t7", debugPrint: false))
        XCTAssertTrue(matcher.isCompleted())
        XCTAssertEqual(matcher.captureValues(deduplicate: false), [Capture(name: "v", value: Data("t7".utf8))])
        let grammar = try grammarFromLarkWithTokenizer(
            larkString: "start: @first", tokenizerInfo: try tokenizerInfo(), namedGrammars: named)
        XCTAssertFalse(grammar.toString().isEmpty)
    }

    func testLarkErrorsAreThrown() {
        XCTAssertThrowsError(try grammarFromLark(larkString: "start: @missing", namedGrammars: NamedGrammars())) { error in
            guard case XgrammarError.Invalid(let message) = error else {
                return XCTFail("unexpected error \(error)")
            }
            XCTAssertTrue(message.contains("missing"), message)
        }
    }
}
