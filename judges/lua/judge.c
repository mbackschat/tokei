/*
 * Judges Lua files with Lua's own lexer, `luaX_next` from llex.c, linked
 * with the rest of Lua's core from its source.
 *
 * Reads paths on stdin and prints, per file, the comment and literal ranges
 * in byte offsets and the lexical errors, as judgecmp's stream expects.
 * Strings, long brackets included, are literals. The lexer skips comments
 * without reporting them, so the text between two tokens holds nothing but
 * whitespace and comments, and the comments are read from it by llex.c's own
 * rules: `--` and a long bracket open a long comment, any other `--` a
 * comment to the end of the line. A first line starting with `#` is skipped
 * as luaL_loadfile skips it (lauxlib.c, `skipcomment`), and counts as a
 * comment. A byte order mark is skipped as luaL_loadfile skips it.
 *
 *     sh build.sh && ./judge-linux < paths
 */
#include "lauxlib.h"
#include "llex.h"
#include "lstate.h"
#include "lstring.h"
#include "ltable.h"
#include "lua.h"
#include "lzio.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
  const char *text;
  size_t len;
  int given;
} Source;

typedef struct {
  size_t (*ranges)[2];
  size_t count, capacity;
} Ranges;

typedef struct {
  const char *text;
  size_t len, start;
  Ranges comments, literals;
} Judged;

static void push(Ranges *r, size_t start, size_t end) {
  if (r->count == r->capacity) {
    r->capacity = r->capacity ? 2 * r->capacity : 64;
    r->ranges = realloc(r->ranges, r->capacity * sizeof *r->ranges);
  }
  r->ranges[r->count][0] = start;
  r->ranges[r->count][1] = end;
  r->count++;
}

static const char *reader(lua_State *L, void *data, size_t *size) {
  Source *s = data;
  (void)L;
  if (s->given) {
    *size = 0;
    return NULL;
  }
  s->given = 1;
  *size = s->len;
  return s->text;
}

/* The level of the long bracket `[==[` at text[i], or -1 when none opens. */
static long bracket_level(const char *text, size_t len, size_t i) {
  size_t j = i + 1;
  if (i >= len || text[i] != '[') return -1;
  while (j < len && text[j] == '=') j++;
  return j < len && text[j] == '[' ? (long)(j - i - 1) : -1;
}

/* Skips whitespace and comments from `i`, recording the comments, as llex.c
 * reads them between tokens, and returns where the next token starts. */
static size_t skip_gap(Judged *j, size_t i) {
  const char *t = j->text;
  while (i < j->len) {
    if (strchr(" \f\t\v\r\n", t[i])) {
      i++;
    } else if (t[i] == '-' && i + 1 < j->len && t[i + 1] == '-') {
      size_t start = i;
      long level = bracket_level(t, j->len, i + 2);
      if (level >= 0) {
        i += 2 + (size_t)level + 2;
        while (i < j->len) {
          if (t[i] == ']') {
            size_t k = i + 1;
            while (k < j->len && t[k] == '=') k++;
            if (k < j->len && t[k] == ']' && (long)(k - i - 1) == level) {
              i = k + 1;
              break;
            }
          }
          i++;
        }
      } else {
        while (i < j->len && t[i] != '\n' && t[i] != '\r') i++;
      }
      push(&j->comments, start, i);
    } else {
      break;
    }
  }
  return i;
}

static int lex(lua_State *L) {
  Judged *j = lua_touserdata(L, 1);
  Source source = {j->text + j->start, j->len - j->start, 0};
  ZIO z;
  Mbuffer buff;
  LexState ls;
  size_t end = j->start;
  memset(&ls, 0, sizeof ls);
  luaZ_init(L, &z, reader, &source);
  luaZ_initbuffer(L, &buff);
  lua_newtable(L);
  ls.h = hvalue(s2v(L->top.p - 1));
  ls.buff = &buff;
  luaX_setinput(L, &ls, &z, luaS_new(L, "judge"), zgetc(&z));
  for (;;) {
    size_t start;
    luaX_next(&ls);
    if (ls.t.token == TK_EOS) break;
    start = skip_gap(j, end);
    /* The lexer has read one character past the token, unless at the end. */
    end = ls.current == EOZ ? j->len : j->start + (size_t)(z.p - source.text) - 1;
    if (ls.t.token == TK_STRING) push(&j->literals, start, end);
  }
  skip_gap(j, end);
  luaZ_freebuffer(L, &buff);
  return 0;
}

static void print_ranges(const Ranges *r) {
  size_t i;
  putchar('[');
  for (i = 0; i < r->count; i++) {
    printf("%s[%zu,%zu]", i ? "," : "", r->ranges[i][0], r->ranges[i][1]);
  }
  putchar(']');
}

static void print_string(const char *s) {
  putchar('"');
  for (; *s; s++) {
    if (*s == '"' || *s == '\\') putchar('\\');
    putchar(*s);
  }
  putchar('"');
}

int main(void) {
  char path[4096];
  lua_State *L = luaL_newstate();
  while (fgets(path, sizeof path, stdin)) {
    FILE *f;
    char *text;
    long size;
    int errors;
    Judged j;
    path[strcspn(path, "\n")] = 0;
    f = fopen(path, "rb");
    if (!f) continue;
    fseek(f, 0, SEEK_END);
    size = ftell(f);
    fseek(f, 0, SEEK_SET);
    text = malloc((size_t)size + 1);
    if (fread(text, 1, (size_t)size, f) != (size_t)size) size = 0;
    fclose(f);
    memset(&j, 0, sizeof j);
    j.text = text;
    j.len = (size_t)size;
    /* luaL_loadfile skips a byte order mark (lauxlib.c, `skipBOM`), and
     * judgecmp's offsets start after it. */
    if (j.len >= 3 && memcmp(text, "\xEF\xBB\xBF", 3) == 0) {
      j.text += 3;
      j.len -= 3;
    }
    if (j.len > 0 && j.text[0] == '#') {
      while (j.start < j.len && j.text[j.start] != '\n') j.start++;
      push(&j.comments, 0, j.start);
    }
    lua_settop(L, 0);
    lua_pushcfunction(L, lex);
    lua_pushlightuserdata(L, &j);
    errors = lua_pcall(L, 1, 0, 0) != LUA_OK;
    printf("{\"path\":");
    print_string(path);
    printf(",\"units\":\"bytes\",\"errors\":%d,\"comments\":", errors);
    print_ranges(&j.comments);
    printf(",\"literals\":");
    print_ranges(&j.literals);
    printf("}\n");
    free(j.comments.ranges);
    free(j.literals.ranges);
    free(text);
    lua_gc(L, LUA_GCCOLLECT);
  }
  lua_close(L);
  return 0;
}
