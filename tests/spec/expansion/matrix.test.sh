# Original POSIX-derived cases. No Oils source copied.
# Reference: https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html
#### arithmetic precedence 1-0
## tags: posix, expansion
printf '%s\n' $((1+2*3))
## stdout: |
7
## status: 0
#### arithmetic precedence 1-1
## tags: posix, expansion
printf '%s\n' $(((1+2)*3))
## stdout: |
9
## status: 0
#### arithmetic precedence 1-2
## tags: posix, expansion
printf '%s\n' $((1*7/2))
## stdout: |
3
## status: 0
#### arithmetic precedence 1-3
## tags: posix, expansion
printf '%s\n' $((1*7%4))
## stdout: |
3
## status: 0
#### arithmetic precedence 1-4
## tags: posix, expansion
printf '%s\n' $((1<<2))
## stdout: |
4
## status: 0
#### arithmetic precedence 1-5
## tags: posix, expansion
printf '%s\n' $((1+1<<2))
## stdout: |
8
## status: 0
#### arithmetic precedence 1-6
## tags: posix, expansion
printf '%s\n' $((1|8))
## stdout: |
9
## status: 0
#### arithmetic precedence 1-7
## tags: posix, expansion
printf '%s\n' $((1^3))
## stdout: |
2
## status: 0
#### arithmetic precedence 1-8
## tags: posix, expansion
printf '%s\n' $((1&3))
## stdout: |
1
## status: 0
#### arithmetic precedence 1-9
## tags: posix, expansion
printf '%s\n' $((1>2 ? 7 : 9))
## stdout: |
9
## status: 0
#### arithmetic precedence 1-10
## tags: posix, expansion
# POSIX 2.6.4/C short-circuiting: dash result is normative; Apple bash 3.2 evaluates the unreachable division (documented reference bug).
printf '%s\n' $((0 && 1/0 || 1>0))
## stdout: |
1
## status: 0
## OK bash: {"stdout": "", "status": 1, "stderr-match": "division by 0"}
#### arithmetic precedence 1-11
## tags: posix, expansion
printf '%s\n' $(((a=1)+(a+=2)))
## stdout: |
4
## status: 0
#### expansion cmd-trailing-newlines 1
## tags: posix, expansion
v=$(printf '1\n\n'); printf '<%s>\n' "$v"
## stdout: |
<1>
## status: 0
#### expansion cmd-inner-newline 1
## tags: posix, expansion
v=$(printf 'a\n1\n'); printf '<%s>\n' "$v"
## stdout: |
<a
1>
## status: 0
#### expansion nested-substitution 1
## tags: posix, expansion
printf '%s\n' "$(printf '%s' "$(printf 1)")"
## stdout: |
1
## status: 0
#### expansion backticks 1
## tags: posix, expansion
printf '%s\n' "`printf 1`"
## stdout: |
1
## status: 0
#### expansion substitution-scope 1
## tags: posix, expansion
v=1; x=$(v=other; printf x); printf '%s:%s\n' "$v" "$x"
## stdout: |
1:x
## status: 0
#### expansion substitution-status 1
## tags: posix, expansion
v=$(exit 1); printf '%s\n' $?
## stdout: |
1
## status: 0
#### expansion tilde-home 1
## tags: posix, expansion
HOME=home1; printf '%s\n' ~
## stdout: |
home1
## status: 0
#### expansion assignment-tilde 1
## tags: posix, expansion
HOME=home1; v=~:~; printf '%s\n' "$v"
## stdout: |
home1:home1
## status: 0
#### expansion short-prefix 1
## tags: posix, expansion
v=a1bcabc; printf '%s\n' "${v#a*}"
## stdout: |
1bcabc
## status: 0
#### expansion long-prefix 1
## tags: posix, expansion
v=a1bcabc; printf '%s\n' "${v##a*}"
## stdout: |

## status: 0
#### expansion short-suffix 1
## tags: posix, expansion
v=abc1abc; printf '%s\n' "${v%c*}"
## stdout: |
abc1ab
## status: 0
#### expansion long-suffix 1
## tags: posix, expansion
v=abc1abc; printf '%s\n' "${v%%c*}"
## stdout: |
ab
## status: 0
#### arithmetic precedence 2-0
## tags: posix, expansion
printf '%s\n' $((2+2*3))
## stdout: |
8
## status: 0
#### arithmetic precedence 2-1
## tags: posix, expansion
printf '%s\n' $(((2+2)*3))
## stdout: |
12
## status: 0
#### arithmetic precedence 2-2
## tags: posix, expansion
printf '%s\n' $((2*7/2))
## stdout: |
7
## status: 0
#### arithmetic precedence 2-3
## tags: posix, expansion
printf '%s\n' $((2*7%4))
## stdout: |
2
## status: 0
#### arithmetic precedence 2-4
## tags: posix, expansion
printf '%s\n' $((2<<2))
## stdout: |
8
## status: 0
#### arithmetic precedence 2-5
## tags: posix, expansion
printf '%s\n' $((2+1<<2))
## stdout: |
12
## status: 0
#### arithmetic precedence 2-6
## tags: posix, expansion
printf '%s\n' $((2|8))
## stdout: |
10
## status: 0
#### arithmetic precedence 2-7
## tags: posix, expansion
printf '%s\n' $((2^3))
## stdout: |
1
## status: 0
#### arithmetic precedence 2-8
## tags: posix, expansion
printf '%s\n' $((2&3))
## stdout: |
2
## status: 0
#### arithmetic precedence 2-9
## tags: posix, expansion
printf '%s\n' $((2>2 ? 7 : 9))
## stdout: |
9
## status: 0
#### arithmetic precedence 2-10
## tags: posix, expansion
# POSIX 2.6.4/C short-circuiting: dash result is normative; Apple bash 3.2 evaluates the unreachable division (documented reference bug).
printf '%s\n' $((0 && 1/0 || 2>0))
## stdout: |
1
## status: 0
## OK bash: {"stdout": "", "status": 1, "stderr-match": "division by 0"}
#### arithmetic precedence 2-11
## tags: posix, expansion
printf '%s\n' $(((a=2)+(a+=2)))
## stdout: |
6
## status: 0
#### expansion cmd-trailing-newlines 2
## tags: posix, expansion
v=$(printf '2\n\n'); printf '<%s>\n' "$v"
## stdout: |
<2>
## status: 0
#### expansion cmd-inner-newline 2
## tags: posix, expansion
v=$(printf 'a\n2\n'); printf '<%s>\n' "$v"
## stdout: |
<a
2>
## status: 0
#### expansion nested-substitution 2
## tags: posix, expansion
printf '%s\n' "$(printf '%s' "$(printf 2)")"
## stdout: |
2
## status: 0
#### expansion backticks 2
## tags: posix, expansion
printf '%s\n' "`printf 2`"
## stdout: |
2
## status: 0
#### expansion substitution-scope 2
## tags: posix, expansion
v=2; x=$(v=other; printf x); printf '%s:%s\n' "$v" "$x"
## stdout: |
2:x
## status: 0
#### expansion substitution-status 2
## tags: posix, expansion
v=$(exit 2); printf '%s\n' $?
## stdout: |
2
## status: 0
#### expansion tilde-home 2
## tags: posix, expansion
HOME=home2; printf '%s\n' ~
## stdout: |
home2
## status: 0
#### expansion assignment-tilde 2
## tags: posix, expansion
HOME=home2; v=~:~; printf '%s\n' "$v"
## stdout: |
home2:home2
## status: 0
#### expansion short-prefix 2
## tags: posix, expansion
v=a2bcabc; printf '%s\n' "${v#a*}"
## stdout: |
2bcabc
## status: 0
#### expansion long-prefix 2
## tags: posix, expansion
v=a2bcabc; printf '%s\n' "${v##a*}"
## stdout: |

## status: 0
#### expansion short-suffix 2
## tags: posix, expansion
v=abc2abc; printf '%s\n' "${v%c*}"
## stdout: |
abc2ab
## status: 0
#### expansion long-suffix 2
## tags: posix, expansion
v=abc2abc; printf '%s\n' "${v%%c*}"
## stdout: |
ab
## status: 0
#### arithmetic precedence 3-0
## tags: posix, expansion
printf '%s\n' $((3+2*3))
## stdout: |
9
## status: 0
#### arithmetic precedence 3-1
## tags: posix, expansion
printf '%s\n' $(((3+2)*3))
## stdout: |
15
## status: 0
#### arithmetic precedence 3-2
## tags: posix, expansion
printf '%s\n' $((3*7/2))
## stdout: |
10
## status: 0
#### arithmetic precedence 3-3
## tags: posix, expansion
printf '%s\n' $((3*7%4))
## stdout: |
1
## status: 0
#### arithmetic precedence 3-4
## tags: posix, expansion
printf '%s\n' $((3<<2))
## stdout: |
12
## status: 0
#### arithmetic precedence 3-5
## tags: posix, expansion
printf '%s\n' $((3+1<<2))
## stdout: |
16
## status: 0
#### arithmetic precedence 3-6
## tags: posix, expansion
printf '%s\n' $((3|8))
## stdout: |
11
## status: 0
#### arithmetic precedence 3-7
## tags: posix, expansion
printf '%s\n' $((3^3))
## stdout: |
0
## status: 0
#### arithmetic precedence 3-8
## tags: posix, expansion
printf '%s\n' $((3&3))
## stdout: |
3
## status: 0
#### arithmetic precedence 3-9
## tags: posix, expansion
printf '%s\n' $((3>2 ? 7 : 9))
## stdout: |
7
## status: 0
#### arithmetic precedence 3-10
## tags: posix, expansion
# POSIX 2.6.4/C short-circuiting: dash result is normative; Apple bash 3.2 evaluates the unreachable division (documented reference bug).
printf '%s\n' $((0 && 1/0 || 3>0))
## stdout: |
1
## status: 0
## OK bash: {"stdout": "", "status": 1, "stderr-match": "division by 0"}
#### arithmetic precedence 3-11
## tags: posix, expansion
printf '%s\n' $(((a=3)+(a+=2)))
## stdout: |
8
## status: 0
#### expansion cmd-trailing-newlines 3
## tags: posix, expansion
v=$(printf '3\n\n'); printf '<%s>\n' "$v"
## stdout: |
<3>
## status: 0
#### expansion cmd-inner-newline 3
## tags: posix, expansion
v=$(printf 'a\n3\n'); printf '<%s>\n' "$v"
## stdout: |
<a
3>
## status: 0
#### expansion nested-substitution 3
## tags: posix, expansion
printf '%s\n' "$(printf '%s' "$(printf 3)")"
## stdout: |
3
## status: 0
#### expansion backticks 3
## tags: posix, expansion
printf '%s\n' "`printf 3`"
## stdout: |
3
## status: 0
#### expansion substitution-scope 3
## tags: posix, expansion
v=3; x=$(v=other; printf x); printf '%s:%s\n' "$v" "$x"
## stdout: |
3:x
## status: 0
#### expansion substitution-status 3
## tags: posix, expansion
v=$(exit 3); printf '%s\n' $?
## stdout: |
3
## status: 0
#### expansion tilde-home 3
## tags: posix, expansion
HOME=home3; printf '%s\n' ~
## stdout: |
home3
## status: 0
#### expansion assignment-tilde 3
## tags: posix, expansion
HOME=home3; v=~:~; printf '%s\n' "$v"
## stdout: |
home3:home3
## status: 0
#### expansion short-prefix 3
## tags: posix, expansion
v=a3bcabc; printf '%s\n' "${v#a*}"
## stdout: |
3bcabc
## status: 0
#### expansion long-prefix 3
## tags: posix, expansion
v=a3bcabc; printf '%s\n' "${v##a*}"
## stdout: |

## status: 0
#### expansion short-suffix 3
## tags: posix, expansion
v=abc3abc; printf '%s\n' "${v%c*}"
## stdout: |
abc3ab
## status: 0
#### expansion long-suffix 3
## tags: posix, expansion
v=abc3abc; printf '%s\n' "${v%%c*}"
## stdout: |
ab
## status: 0
#### arithmetic precedence 4-0
## tags: posix, expansion
printf '%s\n' $((4+2*3))
## stdout: |
10
## status: 0
#### arithmetic precedence 4-1
## tags: posix, expansion
printf '%s\n' $(((4+2)*3))
## stdout: |
18
## status: 0
#### arithmetic precedence 4-2
## tags: posix, expansion
printf '%s\n' $((4*7/2))
## stdout: |
14
## status: 0
#### arithmetic precedence 4-3
## tags: posix, expansion
printf '%s\n' $((4*7%4))
## stdout: |
0
## status: 0
#### arithmetic precedence 4-4
## tags: posix, expansion
printf '%s\n' $((4<<2))
## stdout: |
16
## status: 0
#### arithmetic precedence 4-5
## tags: posix, expansion
printf '%s\n' $((4+1<<2))
## stdout: |
20
## status: 0
#### arithmetic precedence 4-6
## tags: posix, expansion
printf '%s\n' $((4|8))
## stdout: |
12
## status: 0
#### arithmetic precedence 4-7
## tags: posix, expansion
printf '%s\n' $((4^3))
## stdout: |
7
## status: 0
#### arithmetic precedence 4-8
## tags: posix, expansion
printf '%s\n' $((4&3))
## stdout: |
0
## status: 0
#### arithmetic precedence 4-9
## tags: posix, expansion
printf '%s\n' $((4>2 ? 7 : 9))
## stdout: |
7
## status: 0
#### arithmetic precedence 4-10
## tags: posix, expansion
# POSIX 2.6.4/C short-circuiting: dash result is normative; Apple bash 3.2 evaluates the unreachable division (documented reference bug).
printf '%s\n' $((0 && 1/0 || 4>0))
## stdout: |
1
## status: 0
## OK bash: {"stdout": "", "status": 1, "stderr-match": "division by 0"}
#### arithmetic precedence 4-11
## tags: posix, expansion
printf '%s\n' $(((a=4)+(a+=2)))
## stdout: |
10
## status: 0
#### expansion cmd-trailing-newlines 4
## tags: posix, expansion
v=$(printf '4\n\n'); printf '<%s>\n' "$v"
## stdout: |
<4>
## status: 0
#### expansion cmd-inner-newline 4
## tags: posix, expansion
v=$(printf 'a\n4\n'); printf '<%s>\n' "$v"
## stdout: |
<a
4>
## status: 0
#### expansion nested-substitution 4
## tags: posix, expansion
printf '%s\n' "$(printf '%s' "$(printf 4)")"
## stdout: |
4
## status: 0
#### expansion backticks 4
## tags: posix, expansion
printf '%s\n' "`printf 4`"
## stdout: |
4
## status: 0
#### expansion substitution-scope 4
## tags: posix, expansion
v=4; x=$(v=other; printf x); printf '%s:%s\n' "$v" "$x"
## stdout: |
4:x
## status: 0
#### expansion substitution-status 4
## tags: posix, expansion
v=$(exit 4); printf '%s\n' $?
## stdout: |
4
## status: 0
#### expansion tilde-home 4
## tags: posix, expansion
HOME=home4; printf '%s\n' ~
## stdout: |
home4
## status: 0
#### expansion assignment-tilde 4
## tags: posix, expansion
HOME=home4; v=~:~; printf '%s\n' "$v"
## stdout: |
home4:home4
## status: 0
#### expansion short-prefix 4
## tags: posix, expansion
v=a4bcabc; printf '%s\n' "${v#a*}"
## stdout: |
4bcabc
## status: 0
#### expansion long-prefix 4
## tags: posix, expansion
v=a4bcabc; printf '%s\n' "${v##a*}"
## stdout: |

## status: 0
#### expansion short-suffix 4
## tags: posix, expansion
v=abc4abc; printf '%s\n' "${v%c*}"
## stdout: |
abc4ab
## status: 0
#### expansion long-suffix 4
## tags: posix, expansion
v=abc4abc; printf '%s\n' "${v%%c*}"
## stdout: |
ab
## status: 0
#### arithmetic precedence 5-0
## tags: posix, expansion
printf '%s\n' $((5+2*3))
## stdout: |
11
## status: 0
#### arithmetic precedence 5-1
## tags: posix, expansion
printf '%s\n' $(((5+2)*3))
## stdout: |
21
## status: 0
#### arithmetic precedence 5-2
## tags: posix, expansion
printf '%s\n' $((5*7/2))
## stdout: |
17
## status: 0
#### arithmetic precedence 5-3
## tags: posix, expansion
printf '%s\n' $((5*7%4))
## stdout: |
3
## status: 0
#### arithmetic precedence 5-4
## tags: posix, expansion
printf '%s\n' $((5<<2))
## stdout: |
20
## status: 0
#### arithmetic precedence 5-5
## tags: posix, expansion
printf '%s\n' $((5+1<<2))
## stdout: |
24
## status: 0
#### arithmetic precedence 5-6
## tags: posix, expansion
printf '%s\n' $((5|8))
## stdout: |
13
## status: 0
#### arithmetic precedence 5-7
## tags: posix, expansion
printf '%s\n' $((5^3))
## stdout: |
6
## status: 0
#### arithmetic precedence 5-8
## tags: posix, expansion
printf '%s\n' $((5&3))
## stdout: |
1
## status: 0
#### arithmetic precedence 5-9
## tags: posix, expansion
printf '%s\n' $((5>2 ? 7 : 9))
## stdout: |
7
## status: 0
#### arithmetic precedence 5-10
## tags: posix, expansion
# POSIX 2.6.4/C short-circuiting: dash result is normative; Apple bash 3.2 evaluates the unreachable division (documented reference bug).
printf '%s\n' $((0 && 1/0 || 5>0))
## stdout: |
1
## status: 0
## OK bash: {"stdout": "", "status": 1, "stderr-match": "division by 0"}
#### arithmetic precedence 5-11
## tags: posix, expansion
printf '%s\n' $(((a=5)+(a+=2)))
## stdout: |
12
## status: 0
#### expansion cmd-trailing-newlines 5
## tags: posix, expansion
v=$(printf '5\n\n'); printf '<%s>\n' "$v"
## stdout: |
<5>
## status: 0
#### expansion cmd-inner-newline 5
## tags: posix, expansion
v=$(printf 'a\n5\n'); printf '<%s>\n' "$v"
## stdout: |
<a
5>
## status: 0
#### expansion nested-substitution 5
## tags: posix, expansion
printf '%s\n' "$(printf '%s' "$(printf 5)")"
## stdout: |
5
## status: 0
#### expansion backticks 5
## tags: posix, expansion
printf '%s\n' "`printf 5`"
## stdout: |
5
## status: 0
#### expansion substitution-scope 5
## tags: posix, expansion
v=5; x=$(v=other; printf x); printf '%s:%s\n' "$v" "$x"
## stdout: |
5:x
## status: 0
#### expansion substitution-status 5
## tags: posix, expansion
v=$(exit 5); printf '%s\n' $?
## stdout: |
5
## status: 0
#### expansion tilde-home 5
## tags: posix, expansion
HOME=home5; printf '%s\n' ~
## stdout: |
home5
## status: 0
#### expansion assignment-tilde 5
## tags: posix, expansion
HOME=home5; v=~:~; printf '%s\n' "$v"
## stdout: |
home5:home5
## status: 0
#### expansion short-prefix 5
## tags: posix, expansion
v=a5bcabc; printf '%s\n' "${v#a*}"
## stdout: |
5bcabc
## status: 0
#### expansion long-prefix 5
## tags: posix, expansion
v=a5bcabc; printf '%s\n' "${v##a*}"
## stdout: |

## status: 0
#### expansion short-suffix 5
## tags: posix, expansion
v=abc5abc; printf '%s\n' "${v%c*}"
## stdout: |
abc5ab
## status: 0
#### expansion long-suffix 5
## tags: posix, expansion
v=abc5abc; printf '%s\n' "${v%%c*}"
## stdout: |
ab
## status: 0
