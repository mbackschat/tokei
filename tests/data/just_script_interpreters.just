# 15 lines 11 code 2 comments 2 blanks
set script-interpreter := ['/usr/bin/env', 'python3']

[script]
docs:
    '''it's
    # in a docstring
    '''

[script('bash')]
heredoc:
    #!/usr/bin/env python3
    cat <<EOF
    # in a heredoc
    EOF
