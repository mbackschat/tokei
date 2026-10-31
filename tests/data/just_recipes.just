# 31 lines 18 code 10 comments 3 blanks
# Recipes and strings, as just reads them.
retry := '
# inside a string
'
doc := """
  # also a string
"""

build:
    @# a quiet comment
      # an indented comment
    echo "# not a comment"
    # continued \
    still the comment
    cc main.c

script:
    #!/usr/bin/env bash
    cat <<EOF
    # in a heredoc

    EOF
    # a bash comment

py:
    #!/usr/bin/env python3
    """
    # in a docstring
    """
    # a Python comment
