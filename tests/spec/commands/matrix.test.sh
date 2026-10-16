# Original POSIX-derived cases. No Oils source copied.
# Reference: https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html
#### command and-or 0
## tags: posix, commands
false && printf BAD || printf 0; printf '\n'
## stdout: |
0
## status: 0
#### command left-association 0
## tags: posix, commands
true || printf BAD && printf 0; printf '\n'
## stdout: |
0
## status: 0
#### command pipeline-five 0
## tags: posix, commands
printf '0\n' | cat | cat | cat | cat
## stdout: |
0
## status: 0
#### command negated-pipeline 0
## tags: posix, commands
if ! false | false; then printf '0\n'; fi
## stdout: |
0
## status: 0
#### command if-elif 0
## tags: posix, commands
if false; then :; elif true; then printf '0\n'; else :; fi
## stdout: |
0
## status: 0
#### command empty-branch-status 0
## tags: posix, commands
if false; then exit 0; fi; printf '%s\n' $?
## stdout: |
0
## status: 0
#### command while 0
## tags: posix, commands
n=0; while test $n -lt 0; do n=$((n+1)); done; printf '%s\n' $n
## stdout: |
0
## status: 0
#### command until 0
## tags: posix, commands
n=0; until test $n -ge 0; do n=$((n+1)); done; printf '%s\n' $n
## stdout: |
0
## status: 0
#### command for 0
## tags: posix, commands
for x in a 0 b; do printf '<%s>\n' "$x"; done
## stdout: |
<a>
<0>
<b>
## status: 0
#### command for-positional 0
## tags: posix, commands
set -- 0 'a b'; for x; do printf '<%s>\n' "$x"; done
## stdout: |
<0>
<a b>
## status: 0
#### command case 0
## tags: posix, commands
case x0 in (z) printf BAD;; (x[0-9]|other) printf '0\n';; esac
## stdout: |
0
## status: 0
#### command function-args 0
## tags: posix, commands
set -- outer; f(){ printf '%s:%s\n' "$#" "$1"; }; f 0; printf '%s\n' "$1"
## stdout: |
1:0
outer
## status: 0
#### command function-return 0
## tags: posix, commands
f(){ return 0; printf BAD; }; f; printf '%s\n' $?
## stdout: |
0
## status: 0
#### command subshell-scope 0
## tags: posix, commands
v=0; (v=other); printf '%s\n' $v
## stdout: |
0
## status: 0
#### command brace-scope 0
## tags: posix, commands
v=old; { v=0; }; printf '%s\n' $v
## stdout: |
0
## status: 0
#### command break-two 0
## tags: posix, commands
for a in x y; do for b in 1 2; do printf '0\n'; break 2; done; done
## stdout: |
0
## status: 0
#### command and-or 1
## tags: posix, commands
false && printf BAD || printf 1; printf '\n'
## stdout: |
1
## status: 0
#### command left-association 1
## tags: posix, commands
true || printf BAD && printf 1; printf '\n'
## stdout: |
1
## status: 0
#### command pipeline-five 1
## tags: posix, commands
printf '1\n' | cat | cat | cat | cat
## stdout: |
1
## status: 0
#### command negated-pipeline 1
## tags: posix, commands
if ! false | false; then printf '1\n'; fi
## stdout: |
1
## status: 0
#### command if-elif 1
## tags: posix, commands
if false; then :; elif true; then printf '1\n'; else :; fi
## stdout: |
1
## status: 0
#### command empty-branch-status 1
## tags: posix, commands
if false; then exit 1; fi; printf '%s\n' $?
## stdout: |
0
## status: 0
#### command while 1
## tags: posix, commands
n=0; while test $n -lt 1; do n=$((n+1)); done; printf '%s\n' $n
## stdout: |
1
## status: 0
#### command until 1
## tags: posix, commands
n=0; until test $n -ge 1; do n=$((n+1)); done; printf '%s\n' $n
## stdout: |
1
## status: 0
#### command for 1
## tags: posix, commands
for x in a 1 b; do printf '<%s>\n' "$x"; done
## stdout: |
<a>
<1>
<b>
## status: 0
#### command for-positional 1
## tags: posix, commands
set -- 1 'a b'; for x; do printf '<%s>\n' "$x"; done
## stdout: |
<1>
<a b>
## status: 0
#### command case 1
## tags: posix, commands
case x1 in (z) printf BAD;; (x[0-9]|other) printf '1\n';; esac
## stdout: |
1
## status: 0
#### command function-args 1
## tags: posix, commands
set -- outer; f(){ printf '%s:%s\n' "$#" "$1"; }; f 1; printf '%s\n' "$1"
## stdout: |
1:1
outer
## status: 0
#### command function-return 1
## tags: posix, commands
f(){ return 1; printf BAD; }; f; printf '%s\n' $?
## stdout: |
1
## status: 0
#### command subshell-scope 1
## tags: posix, commands
v=1; (v=other); printf '%s\n' $v
## stdout: |
1
## status: 0
#### command brace-scope 1
## tags: posix, commands
v=old; { v=1; }; printf '%s\n' $v
## stdout: |
1
## status: 0
#### command break-two 1
## tags: posix, commands
for a in x y; do for b in 1 2; do printf '1\n'; break 2; done; done
## stdout: |
1
## status: 0
#### command and-or 2
## tags: posix, commands
false && printf BAD || printf 2; printf '\n'
## stdout: |
2
## status: 0
#### command left-association 2
## tags: posix, commands
true || printf BAD && printf 2; printf '\n'
## stdout: |
2
## status: 0
#### command pipeline-five 2
## tags: posix, commands
printf '2\n' | cat | cat | cat | cat
## stdout: |
2
## status: 0
#### command negated-pipeline 2
## tags: posix, commands
if ! false | false; then printf '2\n'; fi
## stdout: |
2
## status: 0
#### command if-elif 2
## tags: posix, commands
if false; then :; elif true; then printf '2\n'; else :; fi
## stdout: |
2
## status: 0
#### command empty-branch-status 2
## tags: posix, commands
if false; then exit 2; fi; printf '%s\n' $?
## stdout: |
0
## status: 0
#### command while 2
## tags: posix, commands
n=0; while test $n -lt 2; do n=$((n+1)); done; printf '%s\n' $n
## stdout: |
2
## status: 0
#### command until 2
## tags: posix, commands
n=0; until test $n -ge 2; do n=$((n+1)); done; printf '%s\n' $n
## stdout: |
2
## status: 0
#### command for 2
## tags: posix, commands
for x in a 2 b; do printf '<%s>\n' "$x"; done
## stdout: |
<a>
<2>
<b>
## status: 0
#### command for-positional 2
## tags: posix, commands
set -- 2 'a b'; for x; do printf '<%s>\n' "$x"; done
## stdout: |
<2>
<a b>
## status: 0
#### command case 2
## tags: posix, commands
case x2 in (z) printf BAD;; (x[0-9]|other) printf '2\n';; esac
## stdout: |
2
## status: 0
#### command function-args 2
## tags: posix, commands
set -- outer; f(){ printf '%s:%s\n' "$#" "$1"; }; f 2; printf '%s\n' "$1"
## stdout: |
1:2
outer
## status: 0
#### command function-return 2
## tags: posix, commands
f(){ return 2; printf BAD; }; f; printf '%s\n' $?
## stdout: |
2
## status: 0
#### command subshell-scope 2
## tags: posix, commands
v=2; (v=other); printf '%s\n' $v
## stdout: |
2
## status: 0
#### command brace-scope 2
## tags: posix, commands
v=old; { v=2; }; printf '%s\n' $v
## stdout: |
2
## status: 0
#### command break-two 2
## tags: posix, commands
for a in x y; do for b in 1 2; do printf '2\n'; break 2; done; done
## stdout: |
2
## status: 0
#### command and-or 3
## tags: posix, commands
false && printf BAD || printf 3; printf '\n'
## stdout: |
3
## status: 0
#### command left-association 3
## tags: posix, commands
true || printf BAD && printf 3; printf '\n'
## stdout: |
3
## status: 0
#### command pipeline-five 3
## tags: posix, commands
printf '3\n' | cat | cat | cat | cat
## stdout: |
3
## status: 0
#### command negated-pipeline 3
## tags: posix, commands
if ! false | false; then printf '3\n'; fi
## stdout: |
3
## status: 0
#### command if-elif 3
## tags: posix, commands
if false; then :; elif true; then printf '3\n'; else :; fi
## stdout: |
3
## status: 0
#### command empty-branch-status 3
## tags: posix, commands
if false; then exit 3; fi; printf '%s\n' $?
## stdout: |
0
## status: 0
#### command while 3
## tags: posix, commands
n=0; while test $n -lt 3; do n=$((n+1)); done; printf '%s\n' $n
## stdout: |
3
## status: 0
#### command until 3
## tags: posix, commands
n=0; until test $n -ge 3; do n=$((n+1)); done; printf '%s\n' $n
## stdout: |
3
## status: 0
#### command for 3
## tags: posix, commands
for x in a 3 b; do printf '<%s>\n' "$x"; done
## stdout: |
<a>
<3>
<b>
## status: 0
#### command for-positional 3
## tags: posix, commands
set -- 3 'a b'; for x; do printf '<%s>\n' "$x"; done
## stdout: |
<3>
<a b>
## status: 0
#### command case 3
## tags: posix, commands
case x3 in (z) printf BAD;; (x[0-9]|other) printf '3\n';; esac
## stdout: |
3
## status: 0
#### command function-args 3
## tags: posix, commands
set -- outer; f(){ printf '%s:%s\n' "$#" "$1"; }; f 3; printf '%s\n' "$1"
## stdout: |
1:3
outer
## status: 0
#### command function-return 3
## tags: posix, commands
f(){ return 3; printf BAD; }; f; printf '%s\n' $?
## stdout: |
3
## status: 0
#### command subshell-scope 3
## tags: posix, commands
v=3; (v=other); printf '%s\n' $v
## stdout: |
3
## status: 0
#### command brace-scope 3
## tags: posix, commands
v=old; { v=3; }; printf '%s\n' $v
## stdout: |
3
## status: 0
#### command break-two 3
## tags: posix, commands
for a in x y; do for b in 1 2; do printf '3\n'; break 2; done; done
## stdout: |
3
## status: 0
#### command and-or 4
## tags: posix, commands
false && printf BAD || printf 4; printf '\n'
## stdout: |
4
## status: 0
#### command left-association 4
## tags: posix, commands
true || printf BAD && printf 4; printf '\n'
## stdout: |
4
## status: 0
#### command pipeline-five 4
## tags: posix, commands
printf '4\n' | cat | cat | cat | cat
## stdout: |
4
## status: 0
#### command negated-pipeline 4
## tags: posix, commands
if ! false | false; then printf '4\n'; fi
## stdout: |
4
## status: 0
#### command if-elif 4
## tags: posix, commands
if false; then :; elif true; then printf '4\n'; else :; fi
## stdout: |
4
## status: 0
#### command empty-branch-status 4
## tags: posix, commands
if false; then exit 4; fi; printf '%s\n' $?
## stdout: |
0
## status: 0
#### command while 4
## tags: posix, commands
n=0; while test $n -lt 4; do n=$((n+1)); done; printf '%s\n' $n
## stdout: |
4
## status: 0
#### command until 4
## tags: posix, commands
n=0; until test $n -ge 4; do n=$((n+1)); done; printf '%s\n' $n
## stdout: |
4
## status: 0
#### command for 4
## tags: posix, commands
for x in a 4 b; do printf '<%s>\n' "$x"; done
## stdout: |
<a>
<4>
<b>
## status: 0
#### command for-positional 4
## tags: posix, commands
set -- 4 'a b'; for x; do printf '<%s>\n' "$x"; done
## stdout: |
<4>
<a b>
## status: 0
#### command case 4
## tags: posix, commands
case x4 in (z) printf BAD;; (x[0-9]|other) printf '4\n';; esac
## stdout: |
4
## status: 0
#### command function-args 4
## tags: posix, commands
set -- outer; f(){ printf '%s:%s\n' "$#" "$1"; }; f 4; printf '%s\n' "$1"
## stdout: |
1:4
outer
## status: 0
#### command function-return 4
## tags: posix, commands
f(){ return 4; printf BAD; }; f; printf '%s\n' $?
## stdout: |
4
## status: 0
#### command subshell-scope 4
## tags: posix, commands
v=4; (v=other); printf '%s\n' $v
## stdout: |
4
## status: 0
#### command brace-scope 4
## tags: posix, commands
v=old; { v=4; }; printf '%s\n' $v
## stdout: |
4
## status: 0
#### command break-two 4
## tags: posix, commands
for a in x y; do for b in 1 2; do printf '4\n'; break 2; done; done
## stdout: |
4
## status: 0
