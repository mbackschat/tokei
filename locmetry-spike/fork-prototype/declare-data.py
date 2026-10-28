"""Apply the spike's language declarations to languages.json.

These are the edits the 2026-10-29 spike made. json.dump reformats the whole
file, so step 1 should transfer the declarations by hand with cited sources.
"""
import json

p = "languages.json"
d = json.load(open(p))
langs = d["languages"]
for name in ["C", "CHeader", "Cpp", "CppHeader", "Rust", "Java", "Go", "Kotlin", "CSharp", "OCaml"]:
    langs[name]["character_literals"] = [["'", "'"]]
langs["Go"]["verbatim_quotes"] = [["`", "`"]]
langs["Rust"]["verbatim_quotes"] = [["r###\\\"", "\\\"###"]] + langs["Rust"]["verbatim_quotes"] + [["r\\\"", "\\\""], ["br\\\"", "\\\""], ["cr\\\"", "\\\""]]
langs["Java"]["quotes"] = [["\\\"\\\"\\\"", "\\\"\\\"\\\""]] + langs["Java"]["quotes"]
json.dump(d, open(p, "w"), indent=2, ensure_ascii=False)
