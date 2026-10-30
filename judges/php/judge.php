<?php
// Judges PHP files with PHP's own tokenizer, token_get_all: reads paths on
// stdin and prints, per file, the comment and literal ranges in byte offsets
// after a byte order mark and whether the file fails to parse, as judgecmp's
// stream expects. The tokens tile the source, so each one's offset is the sum
// of the lengths before it. Strings, heredoc and nowdoc bodies and their
// delimiters are literals; variables and `{$…}` inside strings are code.
// Inline HTML outside the PHP tags is in no range, so its lines are code or
// blank.
const LITERALS = [T_CONSTANT_ENCAPSED_STRING, T_ENCAPSED_AND_WHITESPACE, T_START_HEREDOC, T_END_HEREDOC];
const COMMENTS = [T_COMMENT, T_DOC_COMMENT];

while (($line = fgets(STDIN)) !== false) {
    $path = rtrim($line, "\n");
    $source = file_get_contents($path);
    if ($source === false || !preg_match('//u', $source)) {
        continue;
    }
    if (str_starts_with($source, "\xEF\xBB\xBF")) {
        $source = substr($source, 3);
    }
    $errors = 0;
    try {
        $tokens = token_get_all($source, TOKEN_PARSE);
    } catch (ParseError) {
        $errors = 1;
        $tokens = token_get_all($source);
    }
    $comments = [];
    $literals = [];
    $offset = 0;
    foreach ($tokens as $token) {
        [$kind, $text] = is_array($token) ? [$token[0], $token[1]] : [null, $token];
        $range = [$offset, $offset + strlen($text)];
        if (in_array($kind, COMMENTS, true)) {
            $comments[] = $range;
        } elseif (in_array($kind, LITERALS, true) || $text === '"' || $text === '`') {
            $literals[] = $range;
        }
        $offset += strlen($text);
    }
    echo json_encode(['path' => $path, 'units' => 'bytes', 'errors' => $errors, 'comments' => $comments, 'literals' => $literals], JSON_UNESCAPED_SLASHES), "\n";
}
