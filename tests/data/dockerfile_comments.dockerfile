# 14 lines 9 code 4 comments 1 blanks
FROM alpine
RUN echo "don't panic" && \
    # a comment inside a continued instruction
    echo it's fine
RUN echo a#b # not a comment
COPY <<EOF /etc/motd
# a heredoc line, not a comment
hello
EOF

# a comment
  # an indented comment
ENV A=1
