// Judges Swift files with swift-syntax, the parser the Swift compiler uses,
// as the Xcode toolchain ships it in `usr/lib/swift/host`.
//
// Reads paths on stdin and prints, per file, the comment and literal ranges
// in byte offsets after a byte order mark, as judgecmp's stream expects.
// String and regex literals are literals; the parenthesized expressions of
// string interpolations are code with their own strings and comments. A token
// the lexer flags with an error, or a literal delimiter the parser had to
// invent, makes the file lexically invalid.

import Foundation
import SwiftParser
import SwiftSyntax

final class Ranges: SyntaxVisitor {
    var literals: [[Int]] = []

    override func visit(_ node: StringLiteralExprSyntax) -> SyntaxVisitorContinueKind {
        var start = node.positionAfterSkippingLeadingTrivia.utf8Offset
        for segment in node.segments {
            if let hole = segment.as(ExpressionSegmentSyntax.self) {
                literals.append([start, hole.leftParen.endPositionBeforeTrailingTrivia.utf8Offset])
                start = hole.rightParen.positionAfterSkippingLeadingTrivia.utf8Offset
            }
        }
        literals.append([start, node.endPositionBeforeTrailingTrivia.utf8Offset])
        return .visitChildren
    }

    override func visit(_ node: SimpleStringLiteralExprSyntax) -> SyntaxVisitorContinueKind {
        literals.append([node.positionAfterSkippingLeadingTrivia.utf8Offset, node.endPositionBeforeTrailingTrivia.utf8Offset])
        return .skipChildren
    }

    override func visit(_ node: RegexLiteralExprSyntax) -> SyntaxVisitorContinueKind {
        literals.append([node.positionAfterSkippingLeadingTrivia.utf8Offset, node.endPositionBeforeTrailingTrivia.utf8Offset])
        return .skipChildren
    }
}

let literalTokens: Set<TokenKind> = [.stringSegment(""), .regexLiteralPattern("")]

let delimiters: Set<TokenKind> = [
    .stringQuote, .multilineStringQuote, .singleQuote, .rawStringPoundDelimiter(""), .regexSlash, .regexPoundDelimiter(""),
]

func comments(_ trivia: Trivia, at offset: Int, into ranges: inout [[Int]]) {
    var at = offset
    for piece in trivia {
        switch piece {
        case .lineComment, .blockComment, .docLineComment, .docBlockComment:
            ranges.append([at, at + piece.sourceLength.utf8Length])
        default:
            break
        }
        at += piece.sourceLength.utf8Length
    }
}

func judge(_ path: String) -> String? {
    guard let data = FileManager.default.contents(atPath: path), String(data: data, encoding: .utf8) != nil else { return nil }
    let bom = data.starts(with: [0xEF, 0xBB, 0xBF]) ? 3 : 0
    let tree = data.withUnsafeBytes { Parser.parse(source: $0.bindMemory(to: UInt8.self)) }
    let ranges = Ranges(viewMode: .sourceAccurate)
    ranges.walk(tree)
    var commentRanges: [[Int]] = []
    var errors = 0
    for token in tree.tokens(viewMode: .all) {
        if token.presence == .missing {
            if delimiters.contains(where: { $0.sameKind(as: token.tokenKind) }) { errors += 1 }
            continue
        }
        if token.tokenDiagnostic?.severity == .error { errors += 1 }
        // Guards the visitor: literal text in a node it does not visit
        // would otherwise be judged code without notice.
        let at = token.positionAfterSkippingLeadingTrivia.utf8Offset
        if literalTokens.contains(where: { $0.sameKind(as: token.tokenKind) }),
            !ranges.literals.contains(where: { $0[0] <= at && at < $0[1] })
        {
            // A tree with errors keeps such tokens in unexpected syntax, as
            // in a test file that embeds Objective-C after `//--- Obs.m`.
            if tree.hasError {
                errors += 1
            } else {
                FileHandle.standardError.write("\(path): literal token outside a literal at byte \(at)\n".data(using: .utf8)!)
            }
        }
        comments(token.leadingTrivia, at: token.position.utf8Offset, into: &commentRanges)
        comments(token.trailingTrivia, at: token.endPositionBeforeTrailingTrivia.utf8Offset, into: &commentRanges)
    }
    let shift = { (r: [[Int]]) in r.map { $0.map { $0 - bom } } }
    let object: [String: Any] = [
        "path": path, "units": "bytes", "errors": errors,
        "comments": shift(commentRanges), "literals": shift(ranges.literals.filter { $0[0] < $0[1] }),
    ]
    return String(data: try! JSONSerialization.data(withJSONObject: object), encoding: .utf8)
}

extension TokenKind {
    func sameKind(as other: TokenKind) -> Bool {
        switch (self, other) {
        case (.rawStringPoundDelimiter, .rawStringPoundDelimiter), (.regexPoundDelimiter, .regexPoundDelimiter),
            (.stringSegment, .stringSegment), (.regexLiteralPattern, .regexLiteralPattern):
            return true
        default: return self == other
        }
    }
}

var paths: [String] = []
while let line = readLine() { paths.append(line) }
var results = [String?](repeating: nil, count: paths.count)
// swift-syntax recurses per nesting level, which overflowed the 512 KiB
// stacks of Dispatch's worker threads on the compiler's own test files
// (swiftlang/swift `6d943a0687`, 2026-10-30), so each worker gets 256 MiB.
let lock = NSLock()
var next = 0
let done = DispatchGroup()
results.withUnsafeMutableBufferPointer { out in
    let base = out.baseAddress!
    for _ in 0..<ProcessInfo.processInfo.activeProcessorCount {
        done.enter()
        let worker = Thread {
            while true {
                lock.lock()
                let i = next
                next += 1
                lock.unlock()
                if i >= paths.count { break }
                base[i] = judge(paths[i])
            }
            done.leave()
        }
        worker.stackSize = 256 << 20
        worker.start()
    }
    done.wait()
}
for case let result? in results { print(result) }
