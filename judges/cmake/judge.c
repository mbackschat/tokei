/* Judges CMake files with CMake's own list-file lexer, cmListFileLexer,
 * linked from CMake 4.4.5's source: reads paths on stdin and prints, per
 * file, the comment and literal ranges in byte offsets after a UTF-8 byte
 * order mark, as judgecmp's stream expects. Bracket comments, `#[[...]]`,
 * are tokens; the lexer skips `#` line comments, which are read from the
 * text between tokens, where only whitespace and comments can lie, from a
 * `#` to the end of its line. Quoted and bracket arguments are literals;
 * unquoted arguments, command names and parentheses are code. A bad
 * character, an unterminated bracket or an unterminated string counts as
 * an error (cmake-language(7), "Syntax"). */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "cmListFileLexer.h"

static char *data;
static size_t size;
static size_t *line_starts;
static size_t lines;

static size_t offset_of(long line, long column)
{
  if (line < 1 || (size_t)line > lines) {
    return size;
  }
  size_t at = line_starts[line - 1] + (size_t)(column - 1);
  return at > size ? size : at;
}

static void print_json_string(char const *s)
{
  putchar('"');
  for (; *s; ++s) {
    if (*s == '"' || *s == '\\') {
      putchar('\\');
    }
    putchar(*s);
  }
  putchar('"');
}

static int first = 1;
static void range(size_t start, size_t end)
{
  printf("%s[%zu,%zu]", first ? "" : ",", start, end);
  first = 0;
}

/* The `#` line comments in data[from..to), which holds no token. */
static void gap_comments(size_t from, size_t to)
{
  for (size_t i = from; i < to; ++i) {
    if (data[i] == '#') {
      size_t end = i;
      while (end < to && data[end] != '\n' && data[end] != '\r') {
        ++end;
      }
      range(i, end);
      i = end;
    }
  }
}

static void judge(char const *path)
{
  FILE *f = fopen(path, "rb");
  if (!f) {
    return;
  }
  fseek(f, 0, SEEK_END);
  long length = ftell(f);
  fseek(f, 0, SEEK_SET);
  char *raw = malloc((size_t)length + 1);
  size_t got = fread(raw, 1, (size_t)length, f);
  fclose(f);
  raw[got] = 0;
  size_t bom = got >= 3 && memcmp(raw, "\xEF\xBB\xBF", 3) == 0 ? 3 : 0;
  data = raw + bom;
  size = got - bom;

  lines = 1;
  for (size_t i = 0; i < size; ++i) {
    lines += data[i] == '\n';
  }
  line_starts = malloc(lines * sizeof(size_t));
  line_starts[0] = 0;
  for (size_t i = 0, l = 1; i < size; ++i) {
    if (data[i] == '\n') {
      line_starts[l++] = i + 1;
    }
  }

  int errors = 0;
  size_t comments_cap = 64, literals_cap = 64, nc = 0, nl = 0;
  size_t(*comments)[2] = malloc(comments_cap * sizeof *comments);
  size_t(*literals)[2] = malloc(literals_cap * sizeof *literals);
  size_t(*gaps)[2] = malloc(comments_cap * sizeof *gaps);
  size_t ng = 0, gaps_cap = comments_cap;

  cmListFileLexer *lexer = cmListFileLexer_New();
  cmListFileLexer_SetString(lexer, data, size);
  size_t previous_end = 0;
  cmListFileLexer_Token *token;
  while ((token = cmListFileLexer_Scan(lexer))) {
    size_t start = offset_of(token->line, token->column);
    size_t end = offset_of(cmListFileLexer_GetCurrentLine(lexer),
                           cmListFileLexer_GetCurrentColumn(lexer));
    if (ng == gaps_cap) {
      gaps_cap *= 2;
      gaps = realloc(gaps, gaps_cap * sizeof *gaps);
    }
    gaps[ng][0] = previous_end;
    gaps[ng][1] = start;
    ++ng;
    previous_end = end;
    switch (token->type) {
      case cmListFileLexer_Token_CommentBracket:
        if (nc == comments_cap) {
          comments_cap *= 2;
          comments = realloc(comments, comments_cap * sizeof *comments);
        }
        comments[nc][0] = start;
        comments[nc][1] = end;
        ++nc;
        break;
      case cmListFileLexer_Token_ArgumentQuoted:
      case cmListFileLexer_Token_ArgumentBracket:
        if (nl == literals_cap) {
          literals_cap *= 2;
          literals = realloc(literals, literals_cap * sizeof *literals);
        }
        literals[nl][0] = start;
        literals[nl][1] = end;
        ++nl;
        break;
      case cmListFileLexer_Token_BadCharacter:
      case cmListFileLexer_Token_BadBracket:
      case cmListFileLexer_Token_BadString:
        ++errors;
        break;
      default:
        break;
    }
    if (errors) {
      break;
    }
  }
  if (!errors) {
    if (ng == gaps_cap) {
      gaps = realloc(gaps, (gaps_cap + 1) * sizeof *gaps);
    }
    gaps[ng][0] = previous_end;
    gaps[ng][1] = size;
    ++ng;
  }
  cmListFileLexer_Delete(lexer);

  printf("{\"path\":");
  print_json_string(path);
  printf(",\"units\":\"bytes\",\"errors\":%d,\"comments\":[", errors);
  first = 1;
  for (size_t i = 0; i < ng; ++i) {
    gap_comments(gaps[i][0], gaps[i][1]);
  }
  for (size_t i = 0; i < nc; ++i) {
    range(comments[i][0], comments[i][1]);
  }
  printf("],\"literals\":[");
  first = 1;
  for (size_t i = 0; i < nl; ++i) {
    range(literals[i][0], literals[i][1]);
  }
  printf("]}\n");
  free(comments);
  free(literals);
  free(gaps);
  free(line_starts);
  free(raw);
}

int main(void)
{
  char path[4096];
  while (fgets(path, sizeof path, stdin)) {
    path[strcspn(path, "\n")] = 0;
    judge(path);
  }
  return 0;
}
