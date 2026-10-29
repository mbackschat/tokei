# 15 lines 11 code 4 comments 0 blanks
my @w = qw(
  a'b

  c
);
$v =~ s/'/&#39;/g; # comment after code
$v =~ s{"}
       {x}g;
my $q = q{ it's # not a comment };
# comment
my @parts = split /'/, $v;
# comment
my $z = ($x // '') . 'a/b';
# comment
