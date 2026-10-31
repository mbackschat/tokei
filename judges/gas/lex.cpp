// Lexes GNU-style assembly with LLVM's assembler lexer, the one
// `llvm-mc --as-lex` runs, for the target a file is built for, and, for a
// file the C preprocessor reads first, with clang's lexer in assembler mode
// before it. Reads `MODE TRIPLE PATH` lines on stdin, MODE being `cpp` or
// `plain`, and prints, per file, the comment and literal ranges in byte
// offsets after a byte order mark and whether a comment or string ran to the
// end of the file, as judgecmp's stream expects.
//
// The preprocessor's comments are comments and its directives code; the
// assembler then reads the text with both blanked, as it reads the
// preprocessor's output. A `#` that starts no directive is left to the
// assembler, which reads it as a comment, as `cpp -x assembler-with-cpp`
// passes it on.

#include "clang/Basic/LangOptions.h"
#include "clang/Basic/TokenKinds.h"
#include "clang/Lex/Lexer.h"
#include "llvm/MC/MCAsmInfo.h"
#include "llvm/MC/MCParser/AsmLexer.h"
#include "llvm/MC/MCRegisterInfo.h"
#include "llvm/MC/MCTargetOptions.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/TargetSelect.h"
#include "llvm/TargetParser/Triple.h"

#include <algorithm>
#include <fstream>
#include <iostream>
#include <iterator>
#include <memory>
#include <set>
#include <sstream>
#include <string>
#include <vector>

namespace {

struct Range {
  size_t begin, end;
};

const std::set<std::string> DIRECTIVES = {
    "define", "undef", "include", "include_next", "import", "if",
    "ifdef", "ifndef", "elif", "elifdef", "elifndef", "else", "endif",
    "error", "warning", "pragma", "line", "ident", "sccs", "assert",
    "unassert"};

size_t offset(const clang::Token &token) {
  return token.getLocation().getRawEncoding();
}

// Records the preprocessor's comments and blanks them and its directives in
// `text`.
bool preprocess(std::string &text, std::vector<Range> &comments) {
  clang::LangOptions options;
  options.AsmPreprocessor = true;
  options.LineComment = true;
  clang::Lexer lexer(clang::SourceLocation(), options, text.data(),
                     text.data(), text.data() + text.size());
  lexer.SetCommentRetentionState(true);
  std::vector<clang::Token> tokens;
  for (clang::Token token;;) {
    lexer.LexFromRawLexer(token);
    if (token.is(clang::tok::eof))
      break;
    tokens.push_back(token);
  }
  bool error = false;
  std::vector<Range> blank;
  for (size_t i = 0; i < tokens.size(); ++i) {
    const clang::Token &token = tokens[i];
    size_t begin = offset(token), end = begin + token.getLength();
    if (token.is(clang::tok::comment)) {
      comments.push_back({begin, end});
      blank.push_back({begin, end});
      if (text.compare(begin, 2, "/*") == 0 &&
          (end - begin < 4 || text.compare(end - 2, 2, "*/") != 0))
        error = true;
      continue;
    }
    if (!token.is(clang::tok::hash) || !token.isAtStartOfLine())
      continue;
    size_t next = i + 1;
    while (next < tokens.size() && tokens[next].is(clang::tok::comment) &&
           !tokens[next].isAtStartOfLine())
      ++next;
    // A `#` alone is the null directive; `# 4` is no line marker in an
    // assembly file (clang `Preprocessor::HandleDirective`).
    bool directive = next == tokens.size() || tokens[next].isAtStartOfLine() ||
                     (tokens[next].is(clang::tok::raw_identifier) &&
                      DIRECTIVES.count(tokens[next].getRawIdentifier().str()));
    if (!directive)
      continue;
    size_t last = i;
    while (last + 1 < tokens.size() && !tokens[last + 1].isAtStartOfLine())
      ++last;
    for (size_t j = i; j <= last; ++j)
      if (!tokens[j].is(clang::tok::comment))
        blank.push_back({offset(tokens[j]), offset(tokens[j]) + tokens[j].getLength()});
    i = last;
  }
  for (const Range &range : blank)
    for (size_t k = range.begin; k < range.end; ++k)
      if (text[k] != '\n' && text[k] != '\r')
        text[k] = ' ';
  // Translation phase 2 deletes each backslash before a newline (C23
  // §5.1.1.2), so the assembler reads the two lines as one; spaces keep the
  // offsets.
  for (size_t k = 0; k < text.size(); ++k) {
    if (text[k] != '\\')
      continue;
    size_t end = k + 1;
    while (end < text.size() && (text[end] == ' ' || text[end] == '\t' ||
                                 text[end] == '\f' || text[end] == '\v'))
      ++end;
    if (end < text.size() && text[end] == '\r' && end + 1 < text.size() && text[end + 1] == '\n')
      ++end;
    if (end < text.size() && text[end] == '\n') {
      std::fill(text.begin() + k, text.begin() + end + 1, ' ');
      k = end;
    }
  }
  return error;
}

// Records the assembler's comments and strings in `text`.
bool assemble(const std::string &text, const std::string &triple,
              std::vector<Range> &comments, std::vector<Range> &literals) {
  std::string message;
  const llvm::Target *target = llvm::TargetRegistry::lookupTarget(triple, message);
  if (!target)
    return true;
  llvm::MCTargetOptions options;
  std::unique_ptr<llvm::MCRegisterInfo> registers(target->createMCRegInfo(triple));
  std::unique_ptr<llvm::MCAsmInfo> info(
      target->createMCAsmInfo(*registers, triple, options));
  llvm::AsmLexer lexer(*info);
  lexer.setBuffer(llvm::StringRef(text.data(), text.size()));
  const char *start = text.data();
  bool error = false;
  for (;;) {
    const llvm::AsmToken &token = lexer.Lex();
    if (token.is(llvm::AsmToken::Eof))
      break;
    llvm::StringRef body = token.getString();
    size_t begin = body.data() - start, end = begin + body.size();
    switch (token.getKind()) {
    case llvm::AsmToken::Error:
      if (lexer.getErr().find("unterminated") != std::string::npos)
        error = true;
      break;
    case llvm::AsmToken::Comment:
    case llvm::AsmToken::HashDirective:
      comments.push_back({begin, end});
      break;
    case llvm::AsmToken::EndOfStatement:
      // A line comment ends a statement, and its token is the comment;
      // otherwise the token is the newline or the separator.
      if (!body.empty() && body[0] != '\n' && body[0] != '\r' && body != ";")
        comments.push_back({begin, begin + body.rtrim("\r\n").size()});
      break;
    case llvm::AsmToken::String:
      literals.push_back({begin, end});
      break;
    default:
      break;
    }
  }
  return error;
}

void print(const std::vector<Range> &ranges) {
  std::cout << "[";
  for (size_t i = 0; i < ranges.size(); ++i)
    std::cout << (i ? "," : "") << "[" << ranges[i].begin << "," << ranges[i].end << "]";
  std::cout << "]";
}

std::string quote(const std::string &text) {
  std::string out = "\"";
  for (char c : text) {
    if (c == '"' || c == '\\')
      out += '\\';
    out += c;
  }
  return out + "\"";
}

} // namespace

int main() {
  llvm::InitializeAllTargetInfos();
  llvm::InitializeAllTargetMCs();
  for (std::string line; std::getline(std::cin, line);) {
    std::istringstream fields(line);
    std::string mode, triple, path;
    fields >> mode >> triple;
    std::getline(fields >> std::ws, path);
    std::ifstream file(path, std::ios::binary);
    std::string text((std::istreambuf_iterator<char>(file)), {});
    if (text.compare(0, 3, "\xef\xbb\xbf") == 0)
      text.erase(0, 3);
    std::vector<Range> comments, literals;
    bool error = false;
    if (mode == "cpp")
      error = preprocess(text, comments);
    error |= assemble(text, triple, comments, literals);
    std::cout << "{\"path\":" << quote(path) << ",\"units\":\"bytes\",\"errors\":" << error
              << ",\"comments\":";
    print(comments);
    std::cout << ",\"literals\":";
    print(literals);
    std::cout << "}" << std::endl;
  }
}
