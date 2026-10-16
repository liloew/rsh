# Non-POSIX here-string extension; reference is bash --posix (dash lacks <<<).
cat <<<hello
cat <<<""
v='a b *'; cat <<<$v
cat <<<"$(printf x)$((2+3))"
cat 3<<<fd <&3
{ cat; } <<<group
cat <<<one <<<two
