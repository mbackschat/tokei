// Judges Scala files with the Scala 3 compiler's parser and scanner.
//
// Reads paths on stdin and prints, per file, the comment and literal ranges in
// UTF-16 offsets after a byte order mark, as judgecmp's stream expects. String
// and character tokens are literals; the `$name` and `${…}` parts of
// interpolated strings are code with their own strings and comments. The
// parser drives the scanner, as XML literals are read by the parser, not the
// scanner, and are code. An unclosed string, character or comment makes the
// file lexically invalid; other errors leave the tokens as they are.

import java.nio.charset.{CharacterCodingException, CodingErrorAction, StandardCharsets}
import java.nio.ByteBuffer
import java.nio.file.{Files, Paths}
import scala.collection.mutable

import dotty.tools.dotc.core.Contexts.*
import dotty.tools.dotc.parsing.Parsers.Parser
import dotty.tools.dotc.parsing.Scanners.Scanner
import dotty.tools.dotc.parsing.Tokens.*
import dotty.tools.dotc.reporting.StoreReporter
import dotty.tools.dotc.util.SourceFile

class RecordingParser(source: SourceFile)(using Context) extends Parser(source):
  val literals = mutable.ArrayBuffer.empty[(Int, Int)]
  private var open = -1
  override val in: Scanner = new Scanner(source):
    override def nextToken(): Unit =
      super.nextToken()
      if open >= 0 then
        literals += ((open, lastOffset))
        open = -1
      if token == STRINGLIT || token == STRINGPART || token == CHARLIT then open = offset

def decode(bytes: Array[Byte]): Option[String] =
  try
    Some(
      StandardCharsets.UTF_8.newDecoder
        .onMalformedInput(CodingErrorAction.REPORT)
        .onUnmappableCharacter(CodingErrorAction.REPORT)
        .decode(ByteBuffer.wrap(bytes))
        .toString
    )
  catch case _: CharacterCodingException => None

def ranges(spans: Iterable[(Int, Int)], bom: Int): String =
  spans.map((s, e) => s"[${s - bom},${e - bom}]").mkString("[", ",", "]")

def quote(s: String): String =
  "\"" + s.flatMap {
    case '"' => "\\\""
    case '\\' => "\\\\"
    case c if c < ' ' => f"\\u${c.toInt}%04x"
    case c => c.toString
  } + "\""

def judge(path: String, base: ContextBase): Option[String] =
  decode(Files.readAllBytes(Paths.get(path))).map { text =>
    val reporter = StoreReporter(null)
    given Context = base.initialCtx.fresh.setReporter(reporter)
    val bom = if text.startsWith("﻿") then 1 else 0
    val parser = RecordingParser(SourceFile.virtual(path, text))
    parser.parse()
    // The parser gives up at some errors, such as Scala 2's early
    // definitions; lexing the rest keeps its comments and literals.
    while parser.in.token != EOF do parser.in.nextToken()
    val comments = parser.in.comments.map(c => (c.span.start, c.span.end))
    val errors = reporter.removeBufferedMessages.count(_.message.startsWith("unclosed"))
    s"""{"path":${quote(path)},"units":"utf16","errors":$errors,"comments":${ranges(comments, bom)},"literals":${ranges(parser.literals, bom)}}"""
  }

@main def main(): Unit =
  val base = ContextBase()
  // The parser builds some types, such as `A with B`, from the compiler's
  // definitions, which need the Scala library on the classpath.
  val setup = base.initialCtx.fresh
  setup.setSetting(setup.settings.classpath, System.getProperty("java.class.path"))
  base.initialize()(using setup)
  Iterator.continually(scala.io.StdIn.readLine()).takeWhile(_ != null).foreach { path =>
    try judge(path, base).foreach(println)
    catch case e: Exception => System.err.println(s"$path: judge failed: $e")
  }
