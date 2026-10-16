# Original POSIX-derived cases. No Oils source copied.
# Reference: https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html
#### builtin colon 0
## tags: posix, builtins
: ignored 0
## stdout: |
## status: 0
#### builtin true 0
## tags: posix, builtins
true 0
## stdout: |
## status: 0
#### builtin false 0
## tags: posix, builtins
false 0
## stdout: |
## status: 1
#### builtin printf-decimal 0
## tags: posix, builtins
printf '%05d\n' 0
## stdout: |
00000
## status: 0
#### builtin printf-octal 0
## tags: posix, builtins
printf '%o\n' 8
## stdout: |
10
## status: 0
#### builtin printf-hex 0
## tags: posix, builtins
printf '%X\n' 250
## stdout: |
FA
## status: 0
#### builtin printf-repeat 0
## tags: posix, builtins
printf '<%s>\n' a 0 b
## stdout: |
<a>
<0>
<b>
## status: 0
#### builtin printf-b 0
## tags: posix, builtins
printf '%b' 'a\n0\n'
## stdout: |
a
0
## status: 0
#### builtin echo 0
## tags: posix, builtins
echo a 0
## stdout: |
a 0
## status: 0
#### builtin test-number 0
## tags: posix, builtins
test 0 -eq 0; printf '%s\n' $?
## stdout: |
0
## status: 0
#### builtin test-string 0
## tags: posix, builtins
[ x0 != y0 ]; printf '%s\n' $?
## stdout: |
0
## status: 0
#### builtin test-negation 0
## tags: posix, builtins
test ! '0'; printf '%s\n' $?
## stdout: |
1
## status: 0
#### builtin export 0
## tags: posix, builtins
export VV=0; export -p >vars; test -s vars; printf '%s\n' $VV
## stdout: |
0
## status: 0
#### builtin readonly 0
## tags: posix, builtins
readonly VV=0; readonly -p >vars; test -s vars; printf '%s\n' $VV
## stdout: |
0
## status: 0
#### builtin unset 0
## tags: posix, builtins
VV=0; unset -v VV; printf '%s\n' "${VV-unset}"
## stdout: |
unset
## status: 0
#### builtin shift 0
## tags: posix, builtins
set -- a 0 b; shift; printf '%s:<%s>\n' "$#" "$1"
## stdout: |
2:<0>
## status: 0
#### builtin eval 0
## tags: posix, builtins
v=0; eval 'printf "%s\n" "$v"'
## stdout: |
0
## status: 0
#### builtin exec 0
## tags: posix, builtins
exec /usr/bin/printf "%s\n" 0
## stdout: |
0
## status: 0
#### builtin read 0
## tags: posix, builtins
printf 'a:0:b\n' | { IFS=: read -r a b; printf '<%s>:<%s>\n' "$a" "$b"; }
## stdout: |
<a>:<0:b>
## status: 0
#### builtin getopts 0
## tags: posix, builtins
set -- -a 0; getopts ab: opt; printf '%s:%s\n' $opt $OPTIND
## stdout: |
a:2
## status: 0
#### builtin cd 0
## tags: posix, builtins
mkdir d0; cd d0; printf '%s\n' "${PWD##*/}"
## stdout: |
d0
## status: 0
#### builtin pwd 0
## tags: posix, builtins
mkdir d0; cd d0; cd -P .; test "$(pwd -P)" = "$PWD"; printf '%s\n' $?
## stdout: |
0
## status: 0
#### builtin command-function 0
## tags: posix, builtins
printf(){ false; }; command printf '%s\n' 0
## stdout: |
0
## status: 0
#### builtin umask 0
## tags: posix, builtins
umask 077; printf 0 >f; test -r f; printf '%s\n' $?
## stdout: |
0
## status: 0
#### builtin colon 1
## tags: posix, builtins
: ignored 1
## stdout: |
## status: 0
#### builtin true 1
## tags: posix, builtins
true 1
## stdout: |
## status: 0
#### builtin false 1
## tags: posix, builtins
false 1
## stdout: |
## status: 1
#### builtin printf-decimal 1
## tags: posix, builtins
printf '%05d\n' 1
## stdout: |
00001
## status: 0
#### builtin printf-octal 1
## tags: posix, builtins
printf '%o\n' 9
## stdout: |
11
## status: 0
#### builtin printf-hex 1
## tags: posix, builtins
printf '%X\n' 251
## stdout: |
FB
## status: 0
#### builtin printf-repeat 1
## tags: posix, builtins
printf '<%s>\n' a 1 b
## stdout: |
<a>
<1>
<b>
## status: 0
#### builtin printf-b 1
## tags: posix, builtins
printf '%b' 'a\n1\n'
## stdout: |
a
1
## status: 0
#### builtin echo 1
## tags: posix, builtins
echo a 1
## stdout: |
a 1
## status: 0
#### builtin test-number 1
## tags: posix, builtins
test 1 -eq 1; printf '%s\n' $?
## stdout: |
0
## status: 0
#### builtin test-string 1
## tags: posix, builtins
[ x1 != y1 ]; printf '%s\n' $?
## stdout: |
0
## status: 0
#### builtin test-negation 1
## tags: posix, builtins
test ! '1'; printf '%s\n' $?
## stdout: |
1
## status: 0
#### builtin export 1
## tags: posix, builtins
export VV=1; export -p >vars; test -s vars; printf '%s\n' $VV
## stdout: |
1
## status: 0
#### builtin readonly 1
## tags: posix, builtins
readonly VV=1; readonly -p >vars; test -s vars; printf '%s\n' $VV
## stdout: |
1
## status: 0
#### builtin unset 1
## tags: posix, builtins
VV=1; unset -v VV; printf '%s\n' "${VV-unset}"
## stdout: |
unset
## status: 0
#### builtin shift 1
## tags: posix, builtins
set -- a 1 b; shift; printf '%s:<%s>\n' "$#" "$1"
## stdout: |
2:<1>
## status: 0
#### builtin eval 1
## tags: posix, builtins
v=1; eval 'printf "%s\n" "$v"'
## stdout: |
1
## status: 0
#### builtin exec 1
## tags: posix, builtins
exec /usr/bin/printf "%s\n" 1
## stdout: |
1
## status: 0
#### builtin read 1
## tags: posix, builtins
printf 'a:1:b\n' | { IFS=: read -r a b; printf '<%s>:<%s>\n' "$a" "$b"; }
## stdout: |
<a>:<1:b>
## status: 0
#### builtin getopts 1
## tags: posix, builtins
set -- -a 1; getopts ab: opt; printf '%s:%s\n' $opt $OPTIND
## stdout: |
a:2
## status: 0
#### builtin cd 1
## tags: posix, builtins
mkdir d1; cd d1; printf '%s\n' "${PWD##*/}"
## stdout: |
d1
## status: 0
#### builtin pwd 1
## tags: posix, builtins
mkdir d1; cd d1; cd -P .; test "$(pwd -P)" = "$PWD"; printf '%s\n' $?
## stdout: |
0
## status: 0
#### builtin command-function 1
## tags: posix, builtins
printf(){ false; }; command printf '%s\n' 1
## stdout: |
1
## status: 0
#### builtin umask 1
## tags: posix, builtins
umask 077; printf 1 >f; test -r f; printf '%s\n' $?
## stdout: |
0
## status: 0
#### builtin colon 2
## tags: posix, builtins
: ignored 2
## stdout: |
## status: 0
#### builtin true 2
## tags: posix, builtins
true 2
## stdout: |
## status: 0
#### builtin false 2
## tags: posix, builtins
false 2
## stdout: |
## status: 1
#### builtin printf-decimal 2
## tags: posix, builtins
printf '%05d\n' 2
## stdout: |
00002
## status: 0
#### builtin printf-octal 2
## tags: posix, builtins
printf '%o\n' 10
## stdout: |
12
## status: 0
#### builtin printf-hex 2
## tags: posix, builtins
printf '%X\n' 252
## stdout: |
FC
## status: 0
#### builtin printf-repeat 2
## tags: posix, builtins
printf '<%s>\n' a 2 b
## stdout: |
<a>
<2>
<b>
## status: 0
#### builtin printf-b 2
## tags: posix, builtins
printf '%b' 'a\n2\n'
## stdout: |
a
2
## status: 0
#### builtin echo 2
## tags: posix, builtins
echo a 2
## stdout: |
a 2
## status: 0
#### builtin test-number 2
## tags: posix, builtins
test 2 -eq 2; printf '%s\n' $?
## stdout: |
0
## status: 0
#### builtin test-string 2
## tags: posix, builtins
[ x2 != y2 ]; printf '%s\n' $?
## stdout: |
0
## status: 0
#### builtin test-negation 2
## tags: posix, builtins
test ! '2'; printf '%s\n' $?
## stdout: |
1
## status: 0
#### builtin export 2
## tags: posix, builtins
export VV=2; export -p >vars; test -s vars; printf '%s\n' $VV
## stdout: |
2
## status: 0
#### builtin readonly 2
## tags: posix, builtins
readonly VV=2; readonly -p >vars; test -s vars; printf '%s\n' $VV
## stdout: |
2
## status: 0
#### builtin unset 2
## tags: posix, builtins
VV=2; unset -v VV; printf '%s\n' "${VV-unset}"
## stdout: |
unset
## status: 0
#### builtin shift 2
## tags: posix, builtins
set -- a 2 b; shift; printf '%s:<%s>\n' "$#" "$1"
## stdout: |
2:<2>
## status: 0
#### builtin eval 2
## tags: posix, builtins
v=2; eval 'printf "%s\n" "$v"'
## stdout: |
2
## status: 0
#### builtin exec 2
## tags: posix, builtins
exec /usr/bin/printf "%s\n" 2
## stdout: |
2
## status: 0
#### builtin read 2
## tags: posix, builtins
printf 'a:2:b\n' | { IFS=: read -r a b; printf '<%s>:<%s>\n' "$a" "$b"; }
## stdout: |
<a>:<2:b>
## status: 0
#### builtin getopts 2
## tags: posix, builtins
set -- -a 2; getopts ab: opt; printf '%s:%s\n' $opt $OPTIND
## stdout: |
a:2
## status: 0
#### builtin cd 2
## tags: posix, builtins
mkdir d2; cd d2; printf '%s\n' "${PWD##*/}"
## stdout: |
d2
## status: 0
#### builtin pwd 2
## tags: posix, builtins
mkdir d2; cd d2; cd -P .; test "$(pwd -P)" = "$PWD"; printf '%s\n' $?
## stdout: |
0
## status: 0
#### builtin command-function 2
## tags: posix, builtins
printf(){ false; }; command printf '%s\n' 2
## stdout: |
2
## status: 0
#### builtin umask 2
## tags: posix, builtins
umask 077; printf 2 >f; test -r f; printf '%s\n' $?
## stdout: |
0
## status: 0
#### builtin colon 3
## tags: posix, builtins
: ignored 3
## stdout: |
## status: 0
#### builtin true 3
## tags: posix, builtins
true 3
## stdout: |
## status: 0
#### builtin false 3
## tags: posix, builtins
false 3
## stdout: |
## status: 1
#### builtin printf-decimal 3
## tags: posix, builtins
printf '%05d\n' 3
## stdout: |
00003
## status: 0
#### builtin printf-octal 3
## tags: posix, builtins
printf '%o\n' 11
## stdout: |
13
## status: 0
#### builtin printf-hex 3
## tags: posix, builtins
printf '%X\n' 253
## stdout: |
FD
## status: 0
#### builtin printf-repeat 3
## tags: posix, builtins
printf '<%s>\n' a 3 b
## stdout: |
<a>
<3>
<b>
## status: 0
#### builtin printf-b 3
## tags: posix, builtins
printf '%b' 'a\n3\n'
## stdout: |
a
3
## status: 0
#### builtin echo 3
## tags: posix, builtins
echo a 3
## stdout: |
a 3
## status: 0
#### builtin test-number 3
## tags: posix, builtins
test 3 -eq 3; printf '%s\n' $?
## stdout: |
0
## status: 0
#### builtin test-string 3
## tags: posix, builtins
[ x3 != y3 ]; printf '%s\n' $?
## stdout: |
0
## status: 0
#### builtin test-negation 3
## tags: posix, builtins
test ! '3'; printf '%s\n' $?
## stdout: |
1
## status: 0
#### builtin export 3
## tags: posix, builtins
export VV=3; export -p >vars; test -s vars; printf '%s\n' $VV
## stdout: |
3
## status: 0
#### builtin readonly 3
## tags: posix, builtins
readonly VV=3; readonly -p >vars; test -s vars; printf '%s\n' $VV
## stdout: |
3
## status: 0
#### builtin unset 3
## tags: posix, builtins
VV=3; unset -v VV; printf '%s\n' "${VV-unset}"
## stdout: |
unset
## status: 0
#### builtin shift 3
## tags: posix, builtins
set -- a 3 b; shift; printf '%s:<%s>\n' "$#" "$1"
## stdout: |
2:<3>
## status: 0
#### builtin eval 3
## tags: posix, builtins
v=3; eval 'printf "%s\n" "$v"'
## stdout: |
3
## status: 0
#### builtin exec 3
## tags: posix, builtins
exec /usr/bin/printf "%s\n" 3
## stdout: |
3
## status: 0
#### builtin read 3
## tags: posix, builtins
printf 'a:3:b\n' | { IFS=: read -r a b; printf '<%s>:<%s>\n' "$a" "$b"; }
## stdout: |
<a>:<3:b>
## status: 0
#### builtin getopts 3
## tags: posix, builtins
set -- -a 3; getopts ab: opt; printf '%s:%s\n' $opt $OPTIND
## stdout: |
a:2
## status: 0
#### builtin cd 3
## tags: posix, builtins
mkdir d3; cd d3; printf '%s\n' "${PWD##*/}"
## stdout: |
d3
## status: 0
#### builtin pwd 3
## tags: posix, builtins
mkdir d3; cd d3; cd -P .; test "$(pwd -P)" = "$PWD"; printf '%s\n' $?
## stdout: |
0
## status: 0
#### builtin command-function 3
## tags: posix, builtins
printf(){ false; }; command printf '%s\n' 3
## stdout: |
3
## status: 0
#### builtin umask 3
## tags: posix, builtins
umask 077; printf 3 >f; test -r f; printf '%s\n' $?
## stdout: |
0
## status: 0
#### builtin colon 4
## tags: posix, builtins
: ignored 4
## stdout: |
## status: 0
#### builtin true 4
## tags: posix, builtins
true 4
## stdout: |
## status: 0
#### builtin false 4
## tags: posix, builtins
false 4
## stdout: |
## status: 1
#### builtin printf-decimal 4
## tags: posix, builtins
printf '%05d\n' 4
## stdout: |
00004
## status: 0
#### builtin printf-octal 4
## tags: posix, builtins
printf '%o\n' 12
## stdout: |
14
## status: 0
#### builtin printf-hex 4
## tags: posix, builtins
printf '%X\n' 254
## stdout: |
FE
## status: 0
#### builtin printf-repeat 4
## tags: posix, builtins
printf '<%s>\n' a 4 b
## stdout: |
<a>
<4>
<b>
## status: 0
#### builtin printf-b 4
## tags: posix, builtins
printf '%b' 'a\n4\n'
## stdout: |
a
4
## status: 0
#### builtin echo 4
## tags: posix, builtins
echo a 4
## stdout: |
a 4
## status: 0
#### builtin test-number 4
## tags: posix, builtins
test 4 -eq 4; printf '%s\n' $?
## stdout: |
0
## status: 0
#### builtin test-string 4
## tags: posix, builtins
[ x4 != y4 ]; printf '%s\n' $?
## stdout: |
0
## status: 0
#### builtin test-negation 4
## tags: posix, builtins
test ! '4'; printf '%s\n' $?
## stdout: |
1
## status: 0
#### builtin export 4
## tags: posix, builtins
export VV=4; export -p >vars; test -s vars; printf '%s\n' $VV
## stdout: |
4
## status: 0
#### builtin readonly 4
## tags: posix, builtins
readonly VV=4; readonly -p >vars; test -s vars; printf '%s\n' $VV
## stdout: |
4
## status: 0
#### builtin unset 4
## tags: posix, builtins
VV=4; unset -v VV; printf '%s\n' "${VV-unset}"
## stdout: |
unset
## status: 0
#### builtin shift 4
## tags: posix, builtins
set -- a 4 b; shift; printf '%s:<%s>\n' "$#" "$1"
## stdout: |
2:<4>
## status: 0
#### builtin eval 4
## tags: posix, builtins
v=4; eval 'printf "%s\n" "$v"'
## stdout: |
4
## status: 0
#### builtin exec 4
## tags: posix, builtins
exec /usr/bin/printf "%s\n" 4
## stdout: |
4
## status: 0
#### builtin read 4
## tags: posix, builtins
printf 'a:4:b\n' | { IFS=: read -r a b; printf '<%s>:<%s>\n' "$a" "$b"; }
## stdout: |
<a>:<4:b>
## status: 0
#### builtin getopts 4
## tags: posix, builtins
set -- -a 4; getopts ab: opt; printf '%s:%s\n' $opt $OPTIND
## stdout: |
a:2
## status: 0
#### builtin cd 4
## tags: posix, builtins
mkdir d4; cd d4; printf '%s\n' "${PWD##*/}"
## stdout: |
d4
## status: 0
#### builtin pwd 4
## tags: posix, builtins
mkdir d4; cd d4; cd -P .; test "$(pwd -P)" = "$PWD"; printf '%s\n' $?
## stdout: |
0
## status: 0
#### builtin command-function 4
## tags: posix, builtins
printf(){ false; }; command printf '%s\n' 4
## stdout: |
4
## status: 0
#### builtin umask 4
## tags: posix, builtins
umask 077; printf 4 >f; test -r f; printf '%s\n' $?
## stdout: |
0
## status: 0
