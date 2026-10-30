/// Judges Dart files with the scanner of the Dart SDK's front end, which the
/// compiler and the analyzer share.
///
/// Reads paths on stdin and prints, per file, the comment and literal ranges
/// in UTF-16 offsets after a byte order mark, as judgecmp's stream expects.
/// String tokens are literals; the `${…}` and `$name` interpolations between
/// them are code with their own strings and comments. An unterminated string
/// or comment makes the file lexically invalid; the scanner's other errors,
/// such as unmatched brackets, leave strings and comments as they are.
library;

import 'dart:convert';
import 'dart:io';

import 'package:_fe_analyzer_shared/src/scanner/error_token.dart';
import 'package:_fe_analyzer_shared/src/scanner/scanner.dart';
import 'package:_fe_analyzer_shared/src/scanner/token.dart';

Map<String, Object>? judge(String path) {
  String text;
  try {
    text = utf8.decode(File(path).readAsBytesSync());
  } on FormatException {
    return null;
  }
  final bom = text.startsWith('\uFEFF') ? 1 : 0;
  final comments = <List<int>>[];
  final literals = <List<int>>[];
  var errors = 0;
  Token? token = scanString(text.substring(bom), includeComments: true).tokens;
  while (token != null) {
    for (Token? c = token.precedingComments; c != null; c = c.next) {
      comments.add([c.offset, c.end]);
    }
    if (token is UnterminatedString || token is UnterminatedToken) {
      errors++;
    } else if (token.type == TokenType.STRING) {
      literals.add([token.offset, token.end]);
    }
    if (token.isEof) break;
    token = token.next;
  }
  return {'path': path, 'units': 'utf16', 'errors': errors, 'comments': comments, 'literals': literals};
}

void main() {
  String? line;
  while ((line = stdin.readLineSync()) != null) {
    final judged = judge(line!);
    if (judged != null) print(jsonEncode(judged));
  }
}
