# 21 lines 15 code 6 comments 0 blanks
cat <<EOF
Don't panic
# not a comment
EOF
# comment
cat <<-'END' | tr a b
	it's $(literal)
	END
# comment
x=$(( 1 << 2 ))
# comment
cat <<A <<"B"
a'
A
# b
B
# comment
y=1 # the here-string below is not a heredoc
z=$(cat <<<"it's")
# comment
