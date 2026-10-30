# Judges Elixir files with Elixir's own tokenizer, `:elixir_tokenizer`, the
# one `Code.string_to_quoted_with_comments/2` runs: reads paths on stdin and
# prints, per file, the comment and literal ranges in byte offsets after a
# byte order mark and whether the tokenizer rejected the file, as judgecmp's
# stream expects.
#
# Strings, charlists, heredocs, sigils, quoted atoms and keywords and
# character literals such as `?#` are literals; `#{…}` holes are code with
# their own literals and comments. The tokenizer records where a token
# starts, not where it ends, so a literal ends where the next token or
# comment starts, less the whitespace before it; a character literal keeps
# its own text. Columns count code points.

defmodule Judge do
  @literals [
    :bin_string,
    :list_string,
    :bin_heredoc,
    :list_heredoc,
    :sigil,
    :atom_quoted,
    :atom_safe,
    :atom_unsafe,
    :kw_identifier_safe,
    :kw_identifier_unsafe
  ]

  def judge(path) do
    source =
      case File.read!(path) do
        <<0xEF, 0xBB, 0xBF, rest::binary>> -> rest
        source -> source
      end

    if String.valid?(source), do: judge(path, source)
  end

  defp judge(path, source) do
    lines = source |> :binary.split("\n", [:global]) |> List.to_tuple()
    starts = lines |> Tuple.to_list() |> Enum.scan(0, &(byte_size(&1) + 1 + &2))
    starts = List.to_tuple([0 | starts])
    pos = fn {line, column} -> offset(lines, starts, line, column) end

    collect = fn line, column, _tokens, comment, _rest ->
      Process.put(:comments, [{line, column, comment} | Process.get(:comments, [])])
    end

    Process.put(:comments, [])

    result =
      :elixir_tokenizer.tokenize(String.to_charlist(source), 1, 1,
        preserve_comments: collect,
        existing_atoms_only: false
      )

    case result do
      {:ok, _line, _column, _warnings, tokens, _terminators} ->
        comments =
          for {line, column, text} <- Process.get(:comments) do
            start = pos.({line, column})
            {start, start + byte_size(List.to_string(text))}
          end

        comment_starts = comments |> Enum.map(&elem(&1, 0)) |> Enum.sort()
        context = %{pos: pos, source: source, comment_starts: comment_starts}
        literals = literals(Enum.reverse(tokens), byte_size(source), context)
        report(path, 0, Enum.sort(comments), Enum.sort(literals))

      {:error, _, _, _, _} ->
        report(path, 1, [], [])
    end
  end

  # The byte offset of a one-based line and code point column.
  defp offset(lines, starts, line, column) do
    text = elem(lines, line - 1)
    prefix = text |> String.slice(0, column - 1) |> byte_size()
    elem(starts, line - 1) + prefix
  end

  defp literals(tokens, bound, context) do
    starts = Enum.map(tokens, &context.pos.(position(&1)))
    ends = Enum.drop(starts, 1) ++ [bound]

    Enum.zip([tokens, starts, ends])
    |> Enum.flat_map(fn {token, start, next} ->
      case token do
        {:char, {_, _, text}, _} ->
          [{start, start + byte_size(List.to_string(text))}]

        _ when elem(token, 0) in @literals ->
          finish = literal_end(start, next, context)
          cut(start, finish, parts(token), context)

        _ ->
          []
      end
    end)
  end

  # Where a literal starting at `start` ends: before the next token or
  # comment, less the whitespace before that.
  defp literal_end(start, next, context) do
    comment = Enum.find(context.comment_starts, next, &(&1 > start))
    trim_end(context.source, min(next, comment), start)
  end

  defp trim_end(source, at, floor) do
    if at > floor and :binary.at(source, at - 1) in ~c" \t\r\n" do
      trim_end(source, at - 1, floor)
    else
      at
    end
  end

  # The literal's ranges with its holes cut out, and the holes' own literals.
  defp cut(start, finish, parts, context) do
    holes = for {{l1, c1, _}, {l2, c2, _}, tokens} <- parts, do: {l1, c1, l2, c2, tokens}

    {ranges, from} =
      Enum.reduce(holes, {[], start}, fn {l1, c1, l2, c2, tokens}, {acc, from} ->
        open = context.pos.({l1, c1})
        close = context.pos.({l2, c2})
        inner = literals(tokens, close, context)
        {[{from, open} | inner ++ acc], close + 1}
      end)

    Enum.reject([{from, finish} | ranges], fn {a, b} -> a >= b end)
  end

  defp parts({:sigil, _, _, parts, _, _, _}), do: parts
  defp parts({:bin_heredoc, _, _, parts}), do: parts
  defp parts({:list_heredoc, _, _, parts}), do: parts
  defp parts({_, _, parts}) when is_list(parts), do: parts
  defp parts(_), do: []

  defp position(token) do
    {line, column, _} = elem(token, 1)
    {line, column}
  end

  defp report(path, errors, comments, literals) do
    pairs = fn ranges -> Enum.map(ranges, &Tuple.to_list/1) end

    %{
      path: path,
      units: "bytes",
      errors: errors,
      comments: pairs.(comments),
      literals: pairs.(literals)
    }
    |> JSON.encode!()
  end
end

IO.stream(:stdio, :line)
|> Stream.map(&String.trim_trailing(&1, "\n"))
|> Task.async_stream(&Judge.judge/1, ordered: false, timeout: :infinity)
|> Stream.each(fn
  {:ok, nil} -> :ok
  {:ok, line} -> IO.puts(line)
end)
|> Stream.run()
