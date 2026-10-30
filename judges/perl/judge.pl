# Judges Perl files with PPI, the Perl parser that Perl::Critic is built on,
# since perl itself can only be asked to parse a file by compiling it, which
# runs its BEGIN blocks. Reads paths on stdin and prints, per file, the
# comment and literal ranges in byte offsets after a byte order mark, as
# judgecmp's stream expects. Comments and POD are
# comments; strings, quote-like operators other than `<FH>`, regular
# expressions and here-document bodies are literals; data after `__END__` or
# `__DATA__` is in no range, so its lines are code or blank, except POD
# blocks. PPI looks for POD after `__END__` but not after `__DATA__`, while
# Pod parsers, `perldoc` among them, read a Pod block wherever a line matches
# `m/\A=[a-zA-Z]/` (perlpodspec, "Pod Definitions"), and modules such as
# Opcode keep their documentation after `__DATA__`; so POD is found there as
# PPI finds it after `__END__` (`PPI::Token::End`). PPI keeps a
# here-document's body apart from its tokens, so the offsets are rebuilt as
# `PPI::Document::serialize` places the body: after the first line break
# that follows the opener, taking as many lines of the source as PPI read,
# since PPI drops the indentation of a `<<~` body. PPI also turns every line
# ending into `\n` (`PPI::Tokenizer::new`), so the offsets are mapped back to
# the source's. A file PPI cannot read, or whose rebuilt text differs from
# the source, counts as one error.
use strict;
use warnings;
use JSON::PP;
use PPI;

my $json = JSON::PP->new->canonical;

sub judge {
    my ($path) = @_;
    open my $fh, '<:raw', $path or return;
    my $source = do { local $/; <$fh> };
    close $fh;
    my $copy = $source;
    return unless utf8::decode($copy);
    $source =~ s/\A\xEF\xBB\xBF//;
    my %judged = (path => $path, units => 'bytes', errors => 0, comments => [], literals => []);
    my $document = PPI::Document->new(\$source);
    unless ($document) {
        $judged{errors} = 1;
        return \%judged;
    }
    (my $text = $source) =~ s/(?:\015{1,2}\012|\015|\012)/\n/g;
    my ($output, $pending) = ('', 0);
    my $add = sub {
        my ($part, $class) = @_;
        my $start = length $output;
        $output .= $part;
        push @{ $judged{$class} }, [$start, length $output] if $class && length $part;
    };
    # The next `$pending` lines of the text: the bodies and terminators of
    # the here-documents opened on the line just ended.
    my $bodies = sub {
        my $start = length $output;
        my $end = $start;
        for (1 .. $pending) {
            my $newline = index $text, "\n", $end;
            $end = $newline < 0 ? length $text : $newline + 1;
        }
        $pending = 0;
        $add->(substr($text, $start, $end - $start), 'literals');
    };
    for my $token ($document->tokens) {
        if ($token->isa('PPI::Token::HereDoc')) {
            $add->($token->content, undef);
            $pending += () = $token->heredoc;
            $pending += 1 if defined $token->{_terminator_line};
            next;
        }
        my $content = $token->content;
        if ($token->isa('PPI::Token::Data')) {
            data_pod($content, length $output, $judged{comments});
        }
        my $class = class_of($token);
        my $newline = index $content, "\n";
        if (!$pending || $newline < 0) {
            $add->($content, $class);
            next;
        }
        $add->(substr($content, 0, $newline + 1), $class);
        $bodies->();
        $add->(substr($content, $newline + 1), $class);
    }
    if ($pending) {
        $add->("\n", undef) unless $output =~ /\n\z/;
        $bodies->();
    }
    if ($output ne $text) {
        $judged{errors} = 1;
    } elsif ($text ne $source) {
        remap(\%judged, $source);
    }
    return \%judged;
}

# Maps the ranges of `judged`, offsets into the source with every line ending
# made `\n`, to offsets into `source`.
sub remap {
    my ($judged, $source) = @_;
    my @offset = (0);
    while ($source =~ /\G(\015{1,2}\012|\015|\012|[^\015\012]+)/gc) {
        my ($part, $end) = ($1, pos $source);
        if ($part =~ /^[\015\012]/) {
            push @offset, $end;
        } else {
            my $base = $offset[-1];
            push @offset, $base + $_ for 1 .. length $part;
        }
    }
    for my $range (@{ $judged->{comments} }, @{ $judged->{literals} }) {
        $_ = $offset[$_] for @$range;
    }
}

# Adds to `comments` the Pod blocks of the data that starts at `offset`:
# from a line starting with `=` and a word other than `cut` through the next
# line starting with `=cut`, or the end.
sub data_pod {
    my ($data, $offset, $comments) = @_;
    my $start;
    while ($data =~ /^(.*(?:\n|\z))/gm) {
        my $line = $1;
        last if $line eq '';
        my $end = pos($data);
        if (!defined $start && $line =~ /^=(\w+)/ && $1 ne 'cut') {
            $start = $end - length $line;
        }
        if (defined $start && $line =~ /^=cut\b/) {
            push @$comments, [$offset + $start, $offset + $end];
            undef $start;
        }
    }
    push @$comments, [$offset + $start, $offset + length $data] if defined $start;
}

sub class_of {
    my ($token) = @_;
    return 'comments' if $token->isa('PPI::Token::Comment') || $token->isa('PPI::Token::Pod');
    return undef if $token->isa('PPI::Token::QuoteLike::Readline');
    return 'literals'
        if $token->isa('PPI::Token::Quote')
        || $token->isa('PPI::Token::QuoteLike')
        || $token->isa('PPI::Token::Regexp');
    return undef;
}

while (my $path = <STDIN>) {
    chomp $path;
    my $judged = judge($path) or next;
    print $json->encode($judged), "\n";
}
