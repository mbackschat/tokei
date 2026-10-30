# 22 lines 16 code 4 comments 2 blanks
Name: example
Version: 1
Summary: Don't count this apostrophe as a quote
# a preamble comment

%description
It's text, and the next line is a comment.
# a description comment

%install
cat > %{buildroot}/x.sh << 'EOF'
#!/bin/sh
# a line of the heredoc
EOF
# a shell comment
%if 0%{?fedora}
echo "fedora"
%endif
%changelog
* Thu Oct 29 2026 A <a@example.com> - 1-1
- Don't break; it's fine
