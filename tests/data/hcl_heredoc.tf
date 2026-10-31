# 19 lines 15 code 3 comments 1 blanks
resource "x" "y" {
  user_data = <<-EOF
    #!/bin/bash
    # not a comment

    echo "${var.name}"
  EOF
  // a comment
  glob = "${replace(var.p, "/*", "")}"
  name = "x"
  escaped = "$${"
  # a comment after an escaped hole
  directive = "%{ if v == "/*" }y%{ endif }"
  after = "x"

  plain = <<EOT
EOT
}
