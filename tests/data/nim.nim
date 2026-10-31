# 13 lines 6 code 6 comments 1 blanks
#[ block
  #[ nested ]# still
]#
let q = '"' # quote
let p = r"C:\" # raw
let g = fmt"{p}\" # generalized
##[ doc ]# still doc
]##
let t = """a"""" # run
let n = 1'u8 + ord('#')

proc f() = discard #[ trailing ]#
