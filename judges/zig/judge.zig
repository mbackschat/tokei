//! Judges Zig sources with Zig's own tokenizer, `std.zig.Tokenizer`, the one
//! the compiler parses with: reads paths from stdin and prints, per file, the
//! comment and literal ranges in byte offsets after a byte order mark and the
//! number of invalid tokens, as judgecmp's stream expects.
//!
//! Doc comments, `///` and `//!`, are tokens. Other `//` comments are skipped
//! with whitespace, so they are found in the gaps between tokens, which hold
//! nothing else; a comment runs to the end of its line. String and character
//! literals and each `\\` line of a multiline string are literals.
const std = @import("std");
const Io = std.Io;

var stdin_buffer: [64 * 1024]u8 = undefined;
var stdout_buffer: [64 * 1024]u8 = undefined;

pub fn main(init: std.process.Init) !void {
    const io = init.io;
    const gpa = init.gpa;
    var stdin_reader = Io.File.stdin().reader(io, &stdin_buffer);
    var stdout_writer = Io.File.stdout().writer(io, &stdout_buffer);
    const in = &stdin_reader.interface;
    const out = &stdout_writer.interface;
    while (try in.takeDelimiter('\n')) |path| {
        if (path.len == 0) continue;
        const source = try Io.Dir.cwd().readFileAllocOptions(io, path, gpa, .unlimited, .of(u8), 0);
        defer gpa.free(source);
        try judge(gpa, out, path, source);
        try out.flush();
    }
}

fn judge(gpa: std.mem.Allocator, out: *Io.Writer, path: []const u8, source: [:0]const u8) !void {
    const bom: usize = if (std.mem.startsWith(u8, source, "\xEF\xBB\xBF")) 3 else 0;
    var comments: std.ArrayList([2]usize) = .empty;
    var literals: std.ArrayList([2]usize) = .empty;
    defer comments.deinit(gpa);
    defer literals.deinit(gpa);
    var errors: usize = 0;
    var tokenizer = std.zig.Tokenizer.init(source);
    var gap = bom;
    while (true) {
        const token = tokenizer.next();
        try gapComments(gpa, &comments, source[0..token.loc.start], gap);
        gap = token.loc.end;
        switch (token.tag) {
            .eof => break,
            .invalid => errors += 1,
            .doc_comment, .container_doc_comment => try comments.append(gpa, .{ token.loc.start, token.loc.end }),
            .string_literal, .char_literal, .multiline_string_literal_line => try literals.append(gpa, .{ token.loc.start, token.loc.end }),
            else => {},
        }
    }
    try out.print("{{\"path\":{f},\"units\":\"bytes\",\"errors\":{d},\"comments\":", .{ std.json.fmt(path, .{}), errors });
    try writeRanges(out, comments.items, bom);
    try out.writeAll(",\"literals\":");
    try writeRanges(out, literals.items, bom);
    try out.writeAll("}\n");
}

/// Appends the `//` comments of `text[from..]`, which holds only whitespace
/// and comments.
fn gapComments(gpa: std.mem.Allocator, comments: *std.ArrayList([2]usize), text: []const u8, from: usize) !void {
    var i = from;
    while (i + 1 < text.len) : (i += 1) {
        if (text[i] == '/' and text[i + 1] == '/') {
            const end = std.mem.indexOfScalarPos(u8, text, i, '\n') orelse text.len;
            try comments.append(gpa, .{ i, end });
            i = end;
        }
    }
}

fn writeRanges(out: *Io.Writer, ranges: []const [2]usize, bom: usize) !void {
    try out.writeByte('[');
    for (ranges, 0..) |r, n| {
        if (n > 0) try out.writeByte(',');
        try out.print("[{d},{d}]", .{ r[0] - bom, r[1] - bom });
    }
    try out.writeByte(']');
}
