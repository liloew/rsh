# Original POSIX-derived cases. No Oils source copied.
# Reference: https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html
#### redirection output 0
## tags: posix, redirection-heredoc
printf '%s\n' 0 >f; cat <f
## stdout: |
0
## status: 0
#### redirection append 0
## tags: posix, redirection-heredoc
printf a >f; printf 0 >>f; cat f; printf '\n'
## stdout: |
a0
## status: 0
#### redirection force 0
## tags: posix, redirection-heredoc
printf a >f; set -C; printf 0 >|f; cat f; printf '\n'
## stdout: |
0
## status: 0
#### redirection dup-order-one 0
## tags: posix, redirection-heredoc
{ printf O0; printf E0 >&2; } >f 2>&1; cat f; printf '\n'
## stdout: |
O0E0
## status: 0
#### redirection dup-order-two 0
## tags: posix, redirection-heredoc
{ printf O0; printf E0 >&2; } 2>&1 >f; cat f; printf '\n'
## stdout: |
E0O0
## status: 0
#### redirection persistent-fd 0
## tags: posix, redirection-heredoc
exec 3>f; printf 0 >&3; exec 3>&-; cat f; printf '\n'
## stdout: |
0
## status: 0
#### redirection read-write 0
## tags: posix, redirection-heredoc
printf 0 >f; cat 0<>f; printf '\n'
## stdout: |
0
## status: 0
#### redirection closed-input 0
## tags: posix, redirection-heredoc
printf 0 0<&-; printf '\n'
## stdout: |
0
## status: 0
#### redirection brace-redir 0
## tags: posix, redirection-heredoc
{ printf 0; } >f; cat f; printf '\n'
## stdout: |
0
## status: 0
#### redirection subshell-redir 0
## tags: posix, redirection-heredoc
printf 0 >f; (cat) <f; printf '\n'
## stdout: |
0
## status: 0
#### redirection heredoc 0
## tags: posix, redirection-heredoc
v=0
cat <<EOF
$v
EOF
## stdout: |
0
## status: 0
#### redirection quoted-heredoc 0
## tags: posix, redirection-heredoc
v=0
cat <<'EOF'
$v
EOF
## stdout: |
$v
## status: 0
#### redirection strip-tabs 0
## tags: posix, redirection-heredoc
v=0
cat <<-EOF
	$v
	EOF
## stdout: |
0
## status: 0
#### redirection multiple-documents 0
## tags: posix, redirection-heredoc
cat <<ONE <<TWO
unused
ONE
0
TWO
## stdout: |
0
## status: 0
#### redirection output 1
## tags: posix, redirection-heredoc
printf '%s\n' 1 >f; cat <f
## stdout: |
1
## status: 0
#### redirection append 1
## tags: posix, redirection-heredoc
printf a >f; printf 1 >>f; cat f; printf '\n'
## stdout: |
a1
## status: 0
#### redirection force 1
## tags: posix, redirection-heredoc
printf a >f; set -C; printf 1 >|f; cat f; printf '\n'
## stdout: |
1
## status: 0
#### redirection dup-order-one 1
## tags: posix, redirection-heredoc
{ printf O1; printf E1 >&2; } >f 2>&1; cat f; printf '\n'
## stdout: |
O1E1
## status: 0
#### redirection dup-order-two 1
## tags: posix, redirection-heredoc
{ printf O1; printf E1 >&2; } 2>&1 >f; cat f; printf '\n'
## stdout: |
E1O1
## status: 0
#### redirection persistent-fd 1
## tags: posix, redirection-heredoc
exec 3>f; printf 1 >&3; exec 3>&-; cat f; printf '\n'
## stdout: |
1
## status: 0
#### redirection read-write 1
## tags: posix, redirection-heredoc
printf 1 >f; cat 0<>f; printf '\n'
## stdout: |
1
## status: 0
#### redirection closed-input 1
## tags: posix, redirection-heredoc
printf 1 0<&-; printf '\n'
## stdout: |
1
## status: 0
#### redirection brace-redir 1
## tags: posix, redirection-heredoc
{ printf 1; } >f; cat f; printf '\n'
## stdout: |
1
## status: 0
#### redirection subshell-redir 1
## tags: posix, redirection-heredoc
printf 1 >f; (cat) <f; printf '\n'
## stdout: |
1
## status: 0
#### redirection heredoc 1
## tags: posix, redirection-heredoc
v=1
cat <<EOF
$v
EOF
## stdout: |
1
## status: 0
#### redirection quoted-heredoc 1
## tags: posix, redirection-heredoc
v=1
cat <<'EOF'
$v
EOF
## stdout: |
$v
## status: 0
#### redirection strip-tabs 1
## tags: posix, redirection-heredoc
v=1
cat <<-EOF
	$v
	EOF
## stdout: |
1
## status: 0
#### redirection multiple-documents 1
## tags: posix, redirection-heredoc
cat <<ONE <<TWO
unused
ONE
1
TWO
## stdout: |
1
## status: 0
#### redirection output 2
## tags: posix, redirection-heredoc
printf '%s\n' 2 >f; cat <f
## stdout: |
2
## status: 0
#### redirection append 2
## tags: posix, redirection-heredoc
printf a >f; printf 2 >>f; cat f; printf '\n'
## stdout: |
a2
## status: 0
#### redirection force 2
## tags: posix, redirection-heredoc
printf a >f; set -C; printf 2 >|f; cat f; printf '\n'
## stdout: |
2
## status: 0
#### redirection dup-order-one 2
## tags: posix, redirection-heredoc
{ printf O2; printf E2 >&2; } >f 2>&1; cat f; printf '\n'
## stdout: |
O2E2
## status: 0
#### redirection dup-order-two 2
## tags: posix, redirection-heredoc
{ printf O2; printf E2 >&2; } 2>&1 >f; cat f; printf '\n'
## stdout: |
E2O2
## status: 0
#### redirection persistent-fd 2
## tags: posix, redirection-heredoc
exec 3>f; printf 2 >&3; exec 3>&-; cat f; printf '\n'
## stdout: |
2
## status: 0
#### redirection read-write 2
## tags: posix, redirection-heredoc
printf 2 >f; cat 0<>f; printf '\n'
## stdout: |
2
## status: 0
#### redirection closed-input 2
## tags: posix, redirection-heredoc
printf 2 0<&-; printf '\n'
## stdout: |
2
## status: 0
#### redirection brace-redir 2
## tags: posix, redirection-heredoc
{ printf 2; } >f; cat f; printf '\n'
## stdout: |
2
## status: 0
#### redirection subshell-redir 2
## tags: posix, redirection-heredoc
printf 2 >f; (cat) <f; printf '\n'
## stdout: |
2
## status: 0
#### redirection heredoc 2
## tags: posix, redirection-heredoc
v=2
cat <<EOF
$v
EOF
## stdout: |
2
## status: 0
#### redirection quoted-heredoc 2
## tags: posix, redirection-heredoc
v=2
cat <<'EOF'
$v
EOF
## stdout: |
$v
## status: 0
#### redirection strip-tabs 2
## tags: posix, redirection-heredoc
v=2
cat <<-EOF
	$v
	EOF
## stdout: |
2
## status: 0
#### redirection multiple-documents 2
## tags: posix, redirection-heredoc
cat <<ONE <<TWO
unused
ONE
2
TWO
## stdout: |
2
## status: 0
#### redirection output 3
## tags: posix, redirection-heredoc
printf '%s\n' 3 >f; cat <f
## stdout: |
3
## status: 0
#### redirection append 3
## tags: posix, redirection-heredoc
printf a >f; printf 3 >>f; cat f; printf '\n'
## stdout: |
a3
## status: 0
#### redirection force 3
## tags: posix, redirection-heredoc
printf a >f; set -C; printf 3 >|f; cat f; printf '\n'
## stdout: |
3
## status: 0
#### redirection dup-order-one 3
## tags: posix, redirection-heredoc
{ printf O3; printf E3 >&2; } >f 2>&1; cat f; printf '\n'
## stdout: |
O3E3
## status: 0
#### redirection dup-order-two 3
## tags: posix, redirection-heredoc
{ printf O3; printf E3 >&2; } 2>&1 >f; cat f; printf '\n'
## stdout: |
E3O3
## status: 0
#### redirection persistent-fd 3
## tags: posix, redirection-heredoc
exec 3>f; printf 3 >&3; exec 3>&-; cat f; printf '\n'
## stdout: |
3
## status: 0
#### redirection read-write 3
## tags: posix, redirection-heredoc
printf 3 >f; cat 0<>f; printf '\n'
## stdout: |
3
## status: 0
#### redirection closed-input 3
## tags: posix, redirection-heredoc
printf 3 0<&-; printf '\n'
## stdout: |
3
## status: 0
#### redirection brace-redir 3
## tags: posix, redirection-heredoc
{ printf 3; } >f; cat f; printf '\n'
## stdout: |
3
## status: 0
#### redirection subshell-redir 3
## tags: posix, redirection-heredoc
printf 3 >f; (cat) <f; printf '\n'
## stdout: |
3
## status: 0
#### redirection heredoc 3
## tags: posix, redirection-heredoc
v=3
cat <<EOF
$v
EOF
## stdout: |
3
## status: 0
#### redirection quoted-heredoc 3
## tags: posix, redirection-heredoc
v=3
cat <<'EOF'
$v
EOF
## stdout: |
$v
## status: 0
#### redirection strip-tabs 3
## tags: posix, redirection-heredoc
v=3
cat <<-EOF
	$v
	EOF
## stdout: |
3
## status: 0
#### redirection multiple-documents 3
## tags: posix, redirection-heredoc
cat <<ONE <<TWO
unused
ONE
3
TWO
## stdout: |
3
## status: 0
#### redirection output 4
## tags: posix, redirection-heredoc
printf '%s\n' 4 >f; cat <f
## stdout: |
4
## status: 0
#### redirection append 4
## tags: posix, redirection-heredoc
printf a >f; printf 4 >>f; cat f; printf '\n'
## stdout: |
a4
## status: 0
#### redirection force 4
## tags: posix, redirection-heredoc
printf a >f; set -C; printf 4 >|f; cat f; printf '\n'
## stdout: |
4
## status: 0
#### redirection dup-order-one 4
## tags: posix, redirection-heredoc
{ printf O4; printf E4 >&2; } >f 2>&1; cat f; printf '\n'
## stdout: |
O4E4
## status: 0
#### redirection dup-order-two 4
## tags: posix, redirection-heredoc
{ printf O4; printf E4 >&2; } 2>&1 >f; cat f; printf '\n'
## stdout: |
E4O4
## status: 0
#### redirection persistent-fd 4
## tags: posix, redirection-heredoc
exec 3>f; printf 4 >&3; exec 3>&-; cat f; printf '\n'
## stdout: |
4
## status: 0
#### redirection read-write 4
## tags: posix, redirection-heredoc
printf 4 >f; cat 0<>f; printf '\n'
## stdout: |
4
## status: 0
#### redirection closed-input 4
## tags: posix, redirection-heredoc
printf 4 0<&-; printf '\n'
## stdout: |
4
## status: 0
#### redirection brace-redir 4
## tags: posix, redirection-heredoc
{ printf 4; } >f; cat f; printf '\n'
## stdout: |
4
## status: 0
#### redirection subshell-redir 4
## tags: posix, redirection-heredoc
printf 4 >f; (cat) <f; printf '\n'
## stdout: |
4
## status: 0
#### redirection heredoc 4
## tags: posix, redirection-heredoc
v=4
cat <<EOF
$v
EOF
## stdout: |
4
## status: 0
#### redirection quoted-heredoc 4
## tags: posix, redirection-heredoc
v=4
cat <<'EOF'
$v
EOF
## stdout: |
$v
## status: 0
#### redirection strip-tabs 4
## tags: posix, redirection-heredoc
v=4
cat <<-EOF
	$v
	EOF
## stdout: |
4
## status: 0
#### redirection multiple-documents 4
## tags: posix, redirection-heredoc
cat <<ONE <<TWO
unused
ONE
4
TWO
## stdout: |
4
## status: 0
