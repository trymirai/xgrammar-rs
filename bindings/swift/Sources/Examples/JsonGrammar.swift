import Foundation
import XGrammar

/// Compile the built-in JSON grammar and accept a sample JSON string.
public func runJsonGrammar() throws {
    // Minimal byte-level vocab: one token per byte 0..=255, plus a stop token.
    var encodedVocab: [String] = (0..<256).map { byte in
        String(bytes: [UInt8(byte)], encoding: .isoLatin1)!
    }
    encodedVocab.append("</s>")

    let tokenizerInfo = try TokenizerInfo(
        encodedVocab: encodedVocab,
        vocabType: 1, // VocabType::BYTE_FALLBACK
        vocabSize: nil,
        stopTokenIds: [256],
        addPrefixSpace: false
    )
    let compiler = GrammarCompiler(
        tokenizerInfo: tokenizerInfo,
        maxThreads: 8,
        cacheEnabled: true,
        cacheLimitBytes: -1
    )
    let compiled = compiler.compileBuiltinJsonGrammar()
    let matcher = GrammarMatcher(
        compiledGrammar: compiled,
        overrideStopTokens: nil,
        terminateWithoutStopToken: false,
        maxRollbackTokens: -1
    )

    let sample = #"{"name":"Ada","role":"student"}"#
    precondition(matcher.acceptString(input: sample, debugPrint: false), "grammar rejected: \(sample)")
    print(sample)
    print("completed=\(matcher.isCompleted()) terminated=\(matcher.isTerminated())")
}
