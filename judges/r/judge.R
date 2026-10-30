# Judges R files with R's own parser, through the parse data that
# `parse(keep.source = TRUE)` records for every token.
#
# Reads paths on stdin and prints, per file, the comment and literal ranges in
# byte offsets after a byte order mark, as judgecmp's stream expects. COMMENT
# tokens are comments and STR_CONST tokens, raw strings included, are
# literals. Parse data columns count characters with tabs expanded to every
# eighth column, and the last column is inclusive. A file R cannot parse has
# no tokens and counts as invalid.

suppressPackageStartupMessages(library(jsonlite))

# Byte offset of each display column of `line`, and the byte after the last.
column_offsets <- function(line, start) {
  chars <- strsplit(line, "", useBytes = FALSE)[[1]]
  offsets <- integer(0)
  column <- 1L
  at <- start
  for (char in chars) {
    width <- if (char == "\t") 8L - ((column - 1L) %% 8L) else 1L
    offsets <- c(offsets, rep(at, width))
    column <- column + width
    at <- at + nchar(char, type = "bytes")
  }
  c(offsets, at)
}

judge <- function(path) {
  bytes <- readBin(path, "raw", file.info(path)$size)
  text <- rawToChar(bytes)
  Encoding(text) <- "UTF-8"
  if (!validUTF8(text)) return(NULL)
  data <- tryCatch(
    getParseData(parse(path, keep.source = TRUE, encoding = "UTF-8")),
    error = function(e) NULL
  )
  if (is.null(data)) {
    return(list(path = path, units = "bytes", errors = 1L, comments = list(), literals = list()))
  }
  # Offsets count from after a byte order mark.
  lines <- strsplit(sub("^﻿", "", text), "\n", fixed = TRUE)[[1]]
  starts <- cumsum(c(0L, nchar(lines, type = "bytes") + 1L))
  columns <- lapply(seq_along(lines), function(i) column_offsets(lines[[i]], starts[[i]]))
  span <- function(row) {
    first <- columns[[row$line1]]
    last <- columns[[row$line2]]
    c(first[[row$col1]], last[[min(row$col2 + 1L, length(last))]])
  }
  ranges <- function(kind) {
    rows <- data[data$terminal & data$token == kind, ]
    lapply(seq_len(nrow(rows)), function(i) span(rows[i, ]))
  }
  list(path = path, units = "bytes", errors = 0L, comments = ranges("COMMENT"), literals = ranges("STR_CONST"))
}

input <- file("stdin")
for (path in readLines(input)) {
  judged <- judge(path)
  if (!is.null(judged)) cat(toJSON(judged, auto_unbox = TRUE), "\n", sep = "")
}
close(input)
