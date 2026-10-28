// 12 lines 8 code 3 comments 1 blanks
int main(void) {
    char q = '"';
    char e = '\'';
    char x = '\x22';
    const char *s = "/*";

    /* real comment */
    // line comment
    return q + e + x + s[0];
}
int n = 1'000;
