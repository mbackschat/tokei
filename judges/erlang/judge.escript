#!/usr/bin/env escript
%% Judges Erlang files with Erlang's own scanner, `erl_scan`, the one the
%% compiler's preprocessor runs: reads paths on stdin and prints, per file,
%% the comment and literal ranges in byte offsets after a byte order mark and
%% whether the scanner rejected the file, as judgecmp's stream expects.
%%
%% Comments are comments; strings, triple-quoted strings, sigils with their
%% prefix and suffix, character literals such as `$%` and quoted atoms are
%% literals. `erl_scan` gives each token's line, its column in characters,
%% and its text, from which the byte range follows.

main(_) ->
    ok = io:setopts(standard_io, [binary]),
    loop().

loop() ->
    case io:get_line("") of
        eof -> ok;
        Line -> judge(string:trim(Line, trailing, "\n")), loop()
    end.

judge(Path) ->
    case file:read_file(Path) of
        {ok, <<16#EF, 16#BB, 16#BF, Rest/binary>>} -> judge(Path, Rest);
        {ok, Bin} -> judge(Path, Bin)
    end.

judge(Path, Bin) ->
    case unicode:characters_to_list(Bin) of
        Chars when is_list(Chars) -> judge(Path, Bin, Chars);
        _ -> skip
    end.

judge(Path, Bin, Chars) ->
    Lines = list_to_tuple(binary:split(Bin, <<"\n">>, [global])),
    Starts = list_to_tuple(starts(tuple_to_list(Lines), 0)),
    Offset = fun({L, C}) ->
        Prefix = lists:sublist(unicode:characters_to_list(element(L, Lines)), C - 1),
        element(L, Starts) + byte_size(unicode:characters_to_binary(Prefix))
    end,
    {Tokens, Errors} =
        case erl_scan:string(Chars, {1, 1}, [return_comments, text]) of
            {ok, Ts, _} -> {Ts, 0};
            {error, _, _} -> {[], 1}
        end,
    {Comments, Literals} = lists:foldl(
        fun(Token, {Cs, Ls}) ->
            case kind(Token) of
                code -> {Cs, Ls};
                Kind ->
                    Start = Offset(erl_scan:location(Token)),
                    End = Start + byte_size(unicode:characters_to_binary(text(Token))),
                    case Kind of
                        comment -> {add({Start, End}, Cs), Ls};
                        literal -> {Cs, add({Start, End}, Ls)}
                    end
            end
        end,
        {[], []},
        Tokens
    ),
    io:format("{\"path\":\"~ts\",\"units\":\"bytes\",\"errors\":~b,\"comments\":[~ts],\"literals\":[~ts]}~n",
              [quote(Path), Errors, ranges(Comments), ranges(Literals)]).

kind(Token) ->
    case {erl_scan:category(Token), text(Token)} of
        {comment, _} -> comment;
        {string, _} -> literal;
        {char, _} -> literal;
        {sigil_prefix, _} -> literal;
        {sigil_suffix, _} -> literal;
        {atom, [$' | _]} -> literal;
        _ -> code
    end.

%% An empty sigil suffix has no text.
text(Token) ->
    case erl_scan:text(Token) of
        undefined -> "";
        Text -> Text
    end.

%% Joins a range to the previous one it touches, as a sigil's prefix, text
%% and suffix are one literal.
add({Start, End}, [{S, Start} | Rest]) -> [{S, End} | Rest];
add(Range, Ranges) -> [Range | Ranges].

starts([], _) -> [];
starts([Line | Rest], Offset) -> [Offset | starts(Rest, Offset + byte_size(Line) + 1)].

ranges(Ranges) ->
    lists:join(",", [io_lib:format("[~b,~b]", [S, E]) || {S, E} <- lists:reverse(Ranges)]).

quote(Path) ->
    string:replace(string:replace(Path, "\\", "\\\\", all), "\"", "\\\"", all).
