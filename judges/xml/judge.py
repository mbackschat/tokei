"""Judges XML files with expat, the parser behind Python's `xml.parsers.expat`.

Reads paths on stdin and prints, per file, the comment and literal ranges in
byte offsets and the number of well-formedness errors, as judgecmp's stream
expects. Comments, those of the internal DTD subset included, are comments;
quoted attribute values, of elements and of the XML declaration, are
literals. Character data, CDATA sections included, is code. Expat reports
where each comment and start tag begins; a tag it accepted is well formed
(XML 1.0 §3.1), so its quoted values are read from the text that follows.
External DTDs are not read, so references to entities they declare are
skipped rather than errors (XML 1.0 §4.1, "Well-formedness constraint:
Entity Declared").
"""

import json
import sys
import xml.parsers.expat


def quoted_values(data, start):
    """The quoted values of the tag that starts at `data[start]`, up to its `>`."""
    values, i, n = [], start, len(data)
    while i < n:
        b = data[i]
        if b in (0x22, 0x27):
            end = data.index(b, i + 1)
            values.append([i, end + 1])
            i = end + 1
            continue
        if b == 0x3E:
            break
        i += 1
    return values


def judge(path):
    with open(path, "rb") as f:
        data = f.read()
    try:
        data.decode("utf-8")
    except UnicodeDecodeError:
        return None
    if data.startswith(b"\xef\xbb\xbf"):
        data = data[3:]
    comments, literals = [], []
    parser = xml.parsers.expat.ParserCreate()
    parser.SetParamEntityParsing(xml.parsers.expat.XML_PARAM_ENTITY_PARSING_NEVER)

    def comment(_text):
        start = parser.CurrentByteIndex
        comments.append([start, data.index(b"-->", start + 4) + 3])

    def tag(*_):
        literals.extend(quoted_values(data, parser.CurrentByteIndex))

    parser.CommentHandler = comment
    parser.StartElementHandler = tag
    parser.XmlDeclHandler = tag
    parser.SkippedEntityHandler = lambda *_: None
    errors = 0
    try:
        parser.Parse(data, True)
    except xml.parsers.expat.ExpatError:
        errors = 1
    return {"path": path, "units": "bytes", "errors": errors, "comments": comments, "literals": literals}


for line in sys.stdin:
    judged = judge(line.rstrip("\n"))
    if judged is not None:
        print(json.dumps(judged))
