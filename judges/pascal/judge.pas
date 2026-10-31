{ Judges Pascal files with Free Pascal's own scanner, fcl-passrc's
  TPascalScanner: reads paths on stdin and prints, per file, the comment and
  literal ranges in byte offsets after a byte order mark and whether the
  scanner rejected the file, as judgecmp's stream expects.

  Comments, compiler directives included, are comments; strings and `#`
  character codes are literals. The scanner reads a file as Free Pascal does
  with `FPC` defined, in FPC mode unless a `$mode` directive, in the file or
  an include file it names, says otherwise, so that a brace comment nests in
  another only in the modes that allow it. Include files are read for the modes and switches
  they set, but only the file's own tokens are reported. Text that an `$if`
  leaves inactive is tokenized too, as a counter that evaluates no symbols
  reads it, while the scanner itself would skip it. JUDGE_MODE names another
  starting mode, such as DELPHI; without it, a file that FPC mode leaves
  invalid is read in Delphi mode. }
program judge;

{$mode objfpc}{$H+}

uses
  SysUtils, Classes, Math, PScanner;

type
  TJudgeScanner = class(TPascalScanner)
  protected
    procedure HandleIncludeFile(Param: TPasScannerString); override;
    function HandleMacro(AIndex: Integer): TToken; override;
    procedure PushStackItem; override;
    procedure PopStackItem; override;
  public
    Depth: Integer;
    function NextToken: TToken;
    procedure LeaveInclude;
  end;

{ An include file that cannot be found sets nothing; the file's own tokens
  are judged all the same. }
procedure TJudgeScanner.HandleIncludeFile(Param: TPasScannerString);
begin
  try
    inherited HandleIncludeFile(Param);
  except
    on EScannerError do ;
  end;
end;

{ A macro's use is read as written, an identifier, as a counter that
  expands no macros reads it, though a `$define m := //` directive makes the
  compiler read each `m` as a comment. }
function TJudgeScanner.HandleMacro(AIndex: Integer): TToken;
begin
  Result := tkIdentifier;
end;

procedure TJudgeScanner.PushStackItem;
begin
  inherited PushStackItem;
  Inc(Depth);
end;

procedure TJudgeScanner.PopStackItem;
begin
  inherited PopStackItem;
  Dec(Depth);
end;

{ Every token, those in inactive text included, which FetchToken drops. }
function TJudgeScanner.NextToken: TToken;
begin
  Result := DoFetchToken;
end;

procedure TJudgeScanner.LeaveInclude;
begin
  PopStackItem;
end;

function IsUtf8(const S: RawByteString): Boolean;
var
  I, N, K: Integer;
  B: Byte;
begin
  I := 1;
  N := Length(S);
  while I <= N do
  begin
    B := Ord(S[I]);
    if B < $80 then K := 0
    else if (B and $E0) = $C0 then K := 1
    else if (B and $F0) = $E0 then K := 2
    else if (B and $F8) = $F0 then K := 3
    else Exit(False);
    if I + K > N then Exit(False);
    while K > 0 do
    begin
      Inc(I);
      if (Ord(S[I]) and $C0) <> $80 then Exit(False);
      Dec(K);
    end;
    Inc(I);
  end;
  Result := True;
end;

function Quote(const S: string): string;
begin
  Result := '"' + StringReplace(StringReplace(S, '\', '\\', [rfReplaceAll]), '"', '\"', [rfReplaceAll]) + '"';
end;

procedure Judge(const Path: string);
var
  Data: RawByteString;
  Stream: TFileStream;
  Starts: array of Integer;
  Bom, I, N, Errors, Start, Last, Row0, Col0, Depth0: Integer;
  Resolver: TFileResolver;
  Scanner: TJudgeScanner;
  Token: TToken;
  Comments, Literals, FpcComments, FpcLiterals, Mode: string;

  { The byte offset of the scanner's column `Col`, counted from 1, of row
    `Row`, after a byte order mark. }
  function Offset(Row, Col: Integer): Integer;
  begin
    Result := Starts[Row - 1] + Col - 1 - Bom;
  end;

  procedure Add(var Ranges: string; A, B: Integer);
  begin
    if Ranges <> '' then Ranges := Ranges + ',';
    Ranges := Ranges + Format('[%d,%d]', [A, B]);
  end;

  { The end of a directive whose include file the scanner opened, which
    leaves its position in that file. }
  function DirectiveEnd(From: Integer): Integer;
  var
    Closer: string;
    At: Integer;
  begin
    if Data[From + Bom + 1] = '{' then Closer := '}' else Closer := '*)';
    At := Pos(Closer, Copy(Data, From + Bom + 1, Length(Data)));
    Result := From + At - 1 + Length(Closer);
  end;

  { Reads the file starting in mode `StartMode`, FPC mode when empty. }
  procedure ReadIn(const StartMode: string);
  begin
    Comments := '';
    Literals := '';
    Errors := 0;
    Resolver := TFileResolver.Create;
    Scanner := TJudgeScanner.Create(Resolver);
    try
      Resolver.UseStreams := True;
      Resolver.BaseDirectory := ExtractFilePath(Path);
      Resolver.AddIncludePath(ExtractFilePath(Path));
      // A `$R` resource directive names a file that only the linker reads.
      Scanner.Options := Scanner.Options + [po_DisableResources];
      Scanner.AddDefine('FPC');
      if StartMode <> '' then Scanner.SetCompilerMode(StartMode);
      try
        Scanner.OpenFile(Path);
        while True do
        begin
          Row0 := Scanner.CurRow;
          // Before it reads the first line, the scanner reports column 0.
          Col0 := Max(Scanner.CurColumn, 1);
          Depth0 := Scanner.Depth;
          Token := Scanner.NextToken;
          if Token = tkEOF then
          begin
            if Scanner.Depth > 0 then
            begin
              Scanner.LeaveInclude;
              Continue;
            end;
            // A comment left open runs to the end of the file, which the
            // compiler rejects; the fetch that ends a file otherwise reads no
            // text, and leaves no position to go by.
            Start := Max(Offset(Row0, Col0), 0);
            if (Depth0 = 0) and (Scanner.CurTokenString <> '') then
            begin
              Add(Comments, Start, N - Bom);
              Errors := 1;
            end;
            Break;
          end;
          if Depth0 > 0 then Continue;
          // A token directly after a byte order mark starts where the mark
          // does, as one fetch reads both.
          Start := Max(Offset(Row0, Col0), 0);
          if Scanner.Depth > Depth0 then
            Last := DirectiveEnd(Start)
          else
            Last := Offset(Scanner.CurRow, Scanner.CurColumn);
          case Token of
            tkComment: Add(Comments, Start, Last);
            tkString, tkStringMultiLine: Add(Literals, Start, Last);
          end;
        end;
      except
        on E: Exception do
        begin
          Errors := 1;
          if GetEnvironmentVariable('JUDGE_ERRORS') <> '' then
            WriteLn(StdErr, Path, ': ', E.Message);
        end;
      end;
    finally
      Scanner.Free;
      Resolver.Free;
    end;
  end;

begin
  Stream := TFileStream.Create(Path, fmOpenRead or fmShareDenyNone);
  try
    SetLength(Data, Stream.Size);
    if Stream.Size > 0 then Stream.ReadBuffer(Data[1], Stream.Size);
  finally
    Stream.Free;
  end;
  if not IsUtf8(Data) then Exit;
  if Copy(Data, 1, 3) = #$EF#$BB#$BF then Bom := 3 else Bom := 0;
  // Rows as the scanner's line reader counts them: CR, LF or CR LF ends one.
  N := Length(Data);
  SetLength(Starts, 1);
  Starts[0] := 0;
  I := 1;
  while I <= N do
  begin
    if (Data[I] = #13) and (I < N) and (Data[I + 1] = #10) then Inc(I);
    if Data[I] in [#10, #13] then
    begin
      SetLength(Starts, Length(Starts) + 1);
      Starts[High(Starts)] := I;
    end;
    Inc(I);
  end;
  Mode := GetEnvironmentVariable('JUDGE_MODE');
  ReadIn(Mode);
  // A file that sets no mode is read in the one its project compiles it in,
  // which a counter cannot see: FPC mode first, and Delphi mode when that
  // leaves the file invalid, as when a brace in a comment would nest.
  if (Errors = 1) and (Mode = '') then
  begin
    FpcComments := Comments;
    FpcLiterals := Literals;
    ReadIn('DELPHI');
    if Errors = 1 then
    begin
      Comments := FpcComments;
      Literals := FpcLiterals;
    end;
  end;
  WriteLn(Format('{"path":%s,"units":"bytes","errors":%d,"comments":[%s],"literals":[%s]}',
    [Quote(Path), Errors, Comments, Literals]));
  Flush(Output);
end;

var
  Line: string;
begin
  while not EOF(Input) do
  begin
    ReadLn(Line);
    if Line <> '' then Judge(Line);
  end;
end.
