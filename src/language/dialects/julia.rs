//! Julia reads a `'` directly after an operand as the postfix adjoint
//! operator, and elsewhere as a character literal's opener (JuliaSyntax
//! `lex_prime`, src/julia/tokenize.jl at JuliaLang/julia `3a0616961a`).

/// Whether the byte before `lines[i]` ends an operand: an identifier, which
/// may hold `!` and any non-ASCII letter, a number, a closing bracket, a
/// field access's `.` or another `'`.
pub(crate) fn follows_operand(lines: &[u8], i: usize) -> bool {
    i > 0
        && matches!(
            lines[i - 1],
            b'a'..=b'z'
                | b'A'..=b'Z'
                | b'0'..=b'9'
                | b'_'
                | b'!'
                | b'.'
                | b')'
                | b']'
                | b'}'
                | b'\''
                | 0x80..=0xFF
        )
}
