(* Judges OCaml files with the compiler's own lexer, compiler-libs' [Lexer]:
   reads paths on stdin and prints, per file, the comment and literal ranges
   in byte offsets after a byte order mark and whether the lexer rejected the
   file, as judgecmp's stream expects.

   Comments, documentation comments included, are comments; strings, quoted
   strings such as [{id|...|id}] and [{%ext|...|}], and character literals
   are literals. *)

let () = Lexer.handle_docstrings := false

let literal_range lexbuf =
  Lexing.(lexeme_start_p lexbuf).pos_cnum, Lexing.(lexeme_end_p lexbuf).pos_cnum

let judge path =
  let text = In_channel.with_open_bin path In_channel.input_all in
  let text =
    if String.starts_with ~prefix:"\xEF\xBB\xBF" text then
      String.sub text 3 (String.length text - 3)
    else text
  in
  if String.is_valid_utf_8 text then begin
    let lexbuf = Lexing.from_string text in
    Location.init lexbuf path;
    Lexer.init ();
    let literals = ref [] in
    let errors =
      try
        let rec loop () =
          match Lexer.token lexbuf with
          | Parser.EOF -> 0
          | Parser.STRING _ | Parser.QUOTED_STRING_EXPR _ | Parser.QUOTED_STRING_ITEM _
          | Parser.CHAR _ ->
              literals := literal_range lexbuf :: !literals;
              loop ()
          | _ -> loop ()
        in
        loop ()
      with Lexer.Error _ -> 1
    in
    let comments =
      Lexer.comments ()
      |> List.map (fun (_, (loc : Location.t)) ->
             loc.loc_start.pos_cnum, loc.loc_end.pos_cnum)
      |> List.sort compare
    in
    let ranges spans =
      String.concat "," (List.map (fun (a, b) -> Printf.sprintf "[%d,%d]" a b) spans)
    in
    let quoted =
      String.concat "\\\\" (String.split_on_char '\\' path)
      |> String.split_on_char '"' |> String.concat "\\\""
    in
    Printf.printf
      "{\"path\":\"%s\",\"units\":\"bytes\",\"errors\":%d,\"comments\":[%s],\"literals\":[%s]}\n%!"
      quoted errors (ranges comments) (ranges (List.rev !literals))
  end

let () =
  try
    while true do
      judge (input_line stdin)
    done
  with End_of_file -> ()
