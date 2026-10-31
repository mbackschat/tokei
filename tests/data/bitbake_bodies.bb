# 35 lines 27 code 5 comments 3 blanks
# A comment line, then a value that spans two lines.
SRC_URI = "file://a.patch \
           file://b.patch"

do_install() {
	cat > ${D}/run <<EOF
#!/bin/sh
# kept in the script
EOF
	x="$(cut -d '=' -f 2 | tr -d '"')"
	# a shell comment
}

def triple(d):
    '''
    Mangle bitbake's names.

    '''
    # a Python comment
    return "x"

python do_check() {
    s = """don't
"""
}
addtask check after do_install # a task comment
do_more() {
	echo "${@'a' if d.getVar('X') else ''}"
}
() {
    x = 'it\'s'
    # a comment
    y = 'z'
}
