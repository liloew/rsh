# Original POSIX-derived cases. No Oils source copied.
# Reference: https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html
#### parameter set value 0
## tags: posix, parameters
v=''; printf '<%s>\n' "$v"
## stdout: |
<>
## status: 0
#### parameter unset-default value 0
## tags: posix, parameters
unset v; printf '<%s>\n' "${v-}"
## stdout: |
<>
## status: 0
#### parameter null-no-colon value 0
## tags: posix, parameters
v=; printf '<%s>\n' "${v-}"
## stdout: |
<>
## status: 0
#### parameter null-colon value 0
## tags: posix, parameters
v=; printf '<%s>\n' "${v:-}"
## stdout: |
<>
## status: 0
#### parameter alternate value 0
## tags: posix, parameters
v=x; printf '<%s>\n' "${v:+}"
## stdout: |
<>
## status: 0
#### parameter no-alternate value 0
## tags: posix, parameters
unset v; printf '<%s>\n' "${v+}"
## stdout: |
<>
## status: 0
#### parameter assign-default value 0
## tags: posix, parameters
unset v; : "${v:=}"; printf '<%s>\n' "$v"
## stdout: |
<>
## status: 0
#### parameter length value 0
## tags: posix, parameters
v=''; printf '%s\n' "${#v}"
## stdout: |
0
## status: 0
#### parameter positional value 0
## tags: posix, parameters
set -- '' two; printf '%s:<%s>:<%s>\n' "$#" "$1" "$2"
## stdout: |
2:<>:<two>
## status: 0
#### parameter quoted-at value 0
## tags: posix, parameters
set -- '' two; printf '<%s>\n' "$@"
## stdout: |
<>
<two>
## status: 0
#### parameter quoted-star value 0
## tags: posix, parameters
set -- '' two; IFS=:; printf '<%s>\n' "$*"
## stdout: |
<:two>
## status: 0
#### parameter status-special value 0
## tags: posix, parameters
(exit 0); printf '%s\n' "$?"
## stdout: |
0
## status: 0
#### parameter set value 1
## tags: posix, parameters
v='word'; printf '<%s>\n' "$v"
## stdout: |
<word>
## status: 0
#### parameter unset-default value 1
## tags: posix, parameters
unset v; printf '<%s>\n' "${v-word}"
## stdout: |
<word>
## status: 0
#### parameter null-no-colon value 1
## tags: posix, parameters
v=; printf '<%s>\n' "${v-word}"
## stdout: |
<>
## status: 0
#### parameter null-colon value 1
## tags: posix, parameters
v=; printf '<%s>\n' "${v:-word}"
## stdout: |
<word>
## status: 0
#### parameter alternate value 1
## tags: posix, parameters
v=x; printf '<%s>\n' "${v:+word}"
## stdout: |
<word>
## status: 0
#### parameter no-alternate value 1
## tags: posix, parameters
unset v; printf '<%s>\n' "${v+word}"
## stdout: |
<>
## status: 0
#### parameter assign-default value 1
## tags: posix, parameters
unset v; : "${v:=word}"; printf '<%s>\n' "$v"
## stdout: |
<word>
## status: 0
#### parameter length value 1
## tags: posix, parameters
v='word'; printf '%s\n' "${#v}"
## stdout: |
4
## status: 0
#### parameter positional value 1
## tags: posix, parameters
set -- 'word' two; printf '%s:<%s>:<%s>\n' "$#" "$1" "$2"
## stdout: |
2:<word>:<two>
## status: 0
#### parameter quoted-at value 1
## tags: posix, parameters
set -- 'word' two; printf '<%s>\n' "$@"
## stdout: |
<word>
<two>
## status: 0
#### parameter quoted-star value 1
## tags: posix, parameters
set -- 'word' two; IFS=:; printf '<%s>\n' "$*"
## stdout: |
<word:two>
## status: 0
#### parameter status-special value 1
## tags: posix, parameters
(exit 1); printf '%s\n' "$?"
## stdout: |
1
## status: 0
#### parameter set value 2
## tags: posix, parameters
v='a b'; printf '<%s>\n' "$v"
## stdout: |
<a b>
## status: 0
#### parameter unset-default value 2
## tags: posix, parameters
unset v; printf '<%s>\n' "${v-a b}"
## stdout: |
<a b>
## status: 0
#### parameter null-no-colon value 2
## tags: posix, parameters
v=; printf '<%s>\n' "${v-a b}"
## stdout: |
<>
## status: 0
#### parameter null-colon value 2
## tags: posix, parameters
v=; printf '<%s>\n' "${v:-a b}"
## stdout: |
<a b>
## status: 0
#### parameter alternate value 2
## tags: posix, parameters
v=x; printf '<%s>\n' "${v:+a b}"
## stdout: |
<a b>
## status: 0
#### parameter no-alternate value 2
## tags: posix, parameters
unset v; printf '<%s>\n' "${v+a b}"
## stdout: |
<>
## status: 0
#### parameter assign-default value 2
## tags: posix, parameters
unset v; : "${v:=a b}"; printf '<%s>\n' "$v"
## stdout: |
<a b>
## status: 0
#### parameter length value 2
## tags: posix, parameters
v='a b'; printf '%s\n' "${#v}"
## stdout: |
3
## status: 0
#### parameter positional value 2
## tags: posix, parameters
set -- 'a b' two; printf '%s:<%s>:<%s>\n' "$#" "$1" "$2"
## stdout: |
2:<a b>:<two>
## status: 0
#### parameter quoted-at value 2
## tags: posix, parameters
set -- 'a b' two; printf '<%s>\n' "$@"
## stdout: |
<a b>
<two>
## status: 0
#### parameter quoted-star value 2
## tags: posix, parameters
set -- 'a b' two; IFS=:; printf '<%s>\n' "$*"
## stdout: |
<a b:two>
## status: 0
#### parameter status-special value 2
## tags: posix, parameters
(exit 2); printf '%s\n' "$?"
## stdout: |
2
## status: 0
#### parameter set value 3
## tags: posix, parameters
v='*'; printf '<%s>\n' "$v"
## stdout: |
<*>
## status: 0
#### parameter unset-default value 3
## tags: posix, parameters
unset v; printf '<%s>\n' "${v-*}"
## stdout: |
<*>
## status: 0
#### parameter null-no-colon value 3
## tags: posix, parameters
v=; printf '<%s>\n' "${v-*}"
## stdout: |
<>
## status: 0
#### parameter null-colon value 3
## tags: posix, parameters
v=; printf '<%s>\n' "${v:-*}"
## stdout: |
<*>
## status: 0
#### parameter alternate value 3
## tags: posix, parameters
v=x; printf '<%s>\n' "${v:+*}"
## stdout: |
<*>
## status: 0
#### parameter no-alternate value 3
## tags: posix, parameters
unset v; printf '<%s>\n' "${v+*}"
## stdout: |
<>
## status: 0
#### parameter assign-default value 3
## tags: posix, parameters
unset v; : "${v:=*}"; printf '<%s>\n' "$v"
## stdout: |
<*>
## status: 0
#### parameter length value 3
## tags: posix, parameters
v='*'; printf '%s\n' "${#v}"
## stdout: |
1
## status: 0
#### parameter positional value 3
## tags: posix, parameters
set -- '*' two; printf '%s:<%s>:<%s>\n' "$#" "$1" "$2"
## stdout: |
2:<*>:<two>
## status: 0
#### parameter quoted-at value 3
## tags: posix, parameters
set -- '*' two; printf '<%s>\n' "$@"
## stdout: |
<*>
<two>
## status: 0
#### parameter quoted-star value 3
## tags: posix, parameters
set -- '*' two; IFS=:; printf '<%s>\n' "$*"
## stdout: |
<*:two>
## status: 0
#### parameter status-special value 3
## tags: posix, parameters
(exit 3); printf '%s\n' "$?"
## stdout: |
3
## status: 0
#### parameter set value 4
## tags: posix, parameters
v='é'; printf '<%s>\n' "$v"
## stdout: |
<é>
## status: 0
#### parameter unset-default value 4
## tags: posix, parameters
unset v; printf '<%s>\n' "${v-é}"
## stdout: |
<é>
## status: 0
#### parameter null-no-colon value 4
## tags: posix, parameters
v=; printf '<%s>\n' "${v-é}"
## stdout: |
<>
## status: 0
#### parameter null-colon value 4
## tags: posix, parameters
v=; printf '<%s>\n' "${v:-é}"
## stdout: |
<é>
## status: 0
#### parameter alternate value 4
## tags: posix, parameters
v=x; printf '<%s>\n' "${v:+é}"
## stdout: |
<é>
## status: 0
#### parameter no-alternate value 4
## tags: posix, parameters
unset v; printf '<%s>\n' "${v+é}"
## stdout: |
<>
## status: 0
#### parameter assign-default value 4
## tags: posix, parameters
unset v; : "${v:=é}"; printf '<%s>\n' "$v"
## stdout: |
<é>
## status: 0
#### parameter length value 4
## tags: posix, parameters
v='é'; printf '%s\n' "${#v}"
## stdout: |
2
## status: 0
#### parameter positional value 4
## tags: posix, parameters
set -- 'é' two; printf '%s:<%s>:<%s>\n' "$#" "$1" "$2"
## stdout: |
2:<é>:<two>
## status: 0
#### parameter quoted-at value 4
## tags: posix, parameters
set -- 'é' two; printf '<%s>\n' "$@"
## stdout: |
<é>
<two>
## status: 0
#### parameter quoted-star value 4
## tags: posix, parameters
set -- 'é' two; IFS=:; printf '<%s>\n' "$*"
## stdout: |
<é:two>
## status: 0
#### parameter status-special value 4
## tags: posix, parameters
(exit 4); printf '%s\n' "$?"
## stdout: |
4
## status: 0
