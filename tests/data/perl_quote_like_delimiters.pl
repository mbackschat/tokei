# 13 lines 11 code 2 comments 0 blanks
if ($opt =~ m;^
    # the option's name
    (\w+)
    ;x) {
}
my $ok = $x =~ / ^
    # leading space
    \s* /x;
my @f = map { m,/, ? $_ : "$dir/$_" } @list;
# a comment after them
my %h = (s => 1, y => 2);
print $h{s}, $h{ y };
