# Original POSIX-derived cases. No Oils source copied.
# Reference: https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html
#### option errexit 0
## tags: posix, options
set -e; false; printf BAD0
## stdout: |
## status: 1
#### option errexit-condition 0
## tags: posix, options
set -e; if false; then :; fi; printf '0\n'
## stdout: |
0
## status: 0
#### option errexit-function-exempt 0
## tags: posix, options
set -e; f(){ false; printf '0\n'; }; if f; then :; fi
## stdout: |
0
## status: 0
#### option nounset-at 0
## tags: posix, options
set -u; set --; printf '%s:0\n' "$#"; for x; do :; done
## stdout: |
0:0
## status: 0
#### option xtrace 0
## tags: posix, options
PS4='trace:'; set -x; printf '0\n'
## stdout: |
0
## status: 0
## stderr-match: trace:
#### option noglob 0
## tags: posix, options
printf x >f0; set -f; printf '%s\n' f*
## stdout: |
f*
## status: 0
#### option noexec 0
## tags: posix, options
set -n; printf BAD0
## stdout: |
## status: 0
#### option noclobber 0
## tags: posix, options
printf old >f; set -C; (printf 0 >f) 2>/dev/null; cat f; printf '\n'
## stdout: |
old
## status: 0
#### option errexit 1
## tags: posix, options
set -e; false; printf BAD1
## stdout: |
## status: 1
#### option errexit-condition 1
## tags: posix, options
set -e; if false; then :; fi; printf '1\n'
## stdout: |
1
## status: 0
#### option errexit-function-exempt 1
## tags: posix, options
set -e; f(){ false; printf '1\n'; }; if f; then :; fi
## stdout: |
1
## status: 0
#### option nounset-at 1
## tags: posix, options
set -u; set --; printf '%s:1\n' "$#"; for x; do :; done
## stdout: |
0:1
## status: 0
#### option xtrace 1
## tags: posix, options
PS4='trace:'; set -x; printf '1\n'
## stdout: |
1
## status: 0
## stderr-match: trace:
#### option noglob 1
## tags: posix, options
printf x >f1; set -f; printf '%s\n' f*
## stdout: |
f*
## status: 0
#### option noexec 1
## tags: posix, options
set -n; printf BAD1
## stdout: |
## status: 0
#### option noclobber 1
## tags: posix, options
printf old >f; set -C; (printf 1 >f) 2>/dev/null; cat f; printf '\n'
## stdout: |
old
## status: 0
#### option errexit 2
## tags: posix, options
set -e; false; printf BAD2
## stdout: |
## status: 1
#### option errexit-condition 2
## tags: posix, options
set -e; if false; then :; fi; printf '2\n'
## stdout: |
2
## status: 0
#### option errexit-function-exempt 2
## tags: posix, options
set -e; f(){ false; printf '2\n'; }; if f; then :; fi
## stdout: |
2
## status: 0
#### option nounset-at 2
## tags: posix, options
set -u; set --; printf '%s:2\n' "$#"; for x; do :; done
## stdout: |
0:2
## status: 0
#### option xtrace 2
## tags: posix, options
PS4='trace:'; set -x; printf '2\n'
## stdout: |
2
## status: 0
## stderr-match: trace:
#### option noglob 2
## tags: posix, options
printf x >f2; set -f; printf '%s\n' f*
## stdout: |
f*
## status: 0
#### option noexec 2
## tags: posix, options
set -n; printf BAD2
## stdout: |
## status: 0
#### option noclobber 2
## tags: posix, options
printf old >f; set -C; (printf 2 >f) 2>/dev/null; cat f; printf '\n'
## stdout: |
old
## status: 0
#### option errexit 3
## tags: posix, options
set -e; false; printf BAD3
## stdout: |
## status: 1
#### option errexit-condition 3
## tags: posix, options
set -e; if false; then :; fi; printf '3\n'
## stdout: |
3
## status: 0
#### option errexit-function-exempt 3
## tags: posix, options
set -e; f(){ false; printf '3\n'; }; if f; then :; fi
## stdout: |
3
## status: 0
#### option nounset-at 3
## tags: posix, options
set -u; set --; printf '%s:3\n' "$#"; for x; do :; done
## stdout: |
0:3
## status: 0
#### option xtrace 3
## tags: posix, options
PS4='trace:'; set -x; printf '3\n'
## stdout: |
3
## status: 0
## stderr-match: trace:
#### option noglob 3
## tags: posix, options
printf x >f3; set -f; printf '%s\n' f*
## stdout: |
f*
## status: 0
#### option noexec 3
## tags: posix, options
set -n; printf BAD3
## stdout: |
## status: 0
#### option noclobber 3
## tags: posix, options
printf old >f; set -C; (printf 3 >f) 2>/dev/null; cat f; printf '\n'
## stdout: |
old
## status: 0
#### option errexit 4
## tags: posix, options
set -e; false; printf BAD4
## stdout: |
## status: 1
#### option errexit-condition 4
## tags: posix, options
set -e; if false; then :; fi; printf '4\n'
## stdout: |
4
## status: 0
#### option errexit-function-exempt 4
## tags: posix, options
set -e; f(){ false; printf '4\n'; }; if f; then :; fi
## stdout: |
4
## status: 0
#### option nounset-at 4
## tags: posix, options
set -u; set --; printf '%s:4\n' "$#"; for x; do :; done
## stdout: |
0:4
## status: 0
#### option xtrace 4
## tags: posix, options
PS4='trace:'; set -x; printf '4\n'
## stdout: |
4
## status: 0
## stderr-match: trace:
#### option noglob 4
## tags: posix, options
printf x >f4; set -f; printf '%s\n' f*
## stdout: |
f*
## status: 0
#### option noexec 4
## tags: posix, options
set -n; printf BAD4
## stdout: |
## status: 0
#### option noclobber 4
## tags: posix, options
printf old >f; set -C; (printf 4 >f) 2>/dev/null; cat f; printf '\n'
## stdout: |
old
## status: 0
#### pipefail extension 1
## tags: extension, options
# Extension: pipefail is supported by bash and rsh; absent in the recorded dash reference.
set -o pipefail; (exit 1) | true; printf '%s\n' $?
## stdout: |
1
## status: 0
## N-I dash: Apple dash-16 has no pipefail option
#### pipefail extension 2
## tags: extension, options
# Extension: pipefail is supported by bash and rsh; absent in the recorded dash reference.
set -o pipefail; (exit 2) | true; printf '%s\n' $?
## stdout: |
2
## status: 0
## N-I dash: Apple dash-16 has no pipefail option
#### pipefail extension 3
## tags: extension, options
# Extension: pipefail is supported by bash and rsh; absent in the recorded dash reference.
set -o pipefail; (exit 3) | true; printf '%s\n' $?
## stdout: |
3
## status: 0
## N-I dash: Apple dash-16 has no pipefail option
#### pipefail extension 4
## tags: extension, options
# Extension: pipefail is supported by bash and rsh; absent in the recorded dash reference.
set -o pipefail; (exit 4) | true; printf '%s\n' $?
## stdout: |
4
## status: 0
## N-I dash: Apple dash-16 has no pipefail option
#### pipefail extension 5
## tags: extension, options
# Extension: pipefail is supported by bash and rsh; absent in the recorded dash reference.
set -o pipefail; (exit 5) | true; printf '%s\n' $?
## stdout: |
5
## status: 0
## N-I dash: Apple dash-16 has no pipefail option
