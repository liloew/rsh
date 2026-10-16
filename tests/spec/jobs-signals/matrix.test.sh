# Original POSIX-derived cases. No Oils source copied.
# Reference: https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html
#### job wait-status 0
## tags: posix, jobs-signals
(exit 0) & p=$!; wait $p; printf '%s\n' $?
## stdout: |
0
## status: 0
#### job wait-unknown 0
## tags: posix, jobs-signals
wait 900000 2>/dev/null; printf '%s\n' $?
## stdout: |
127
## status: 0
#### job wait-all 0
## tags: posix, jobs-signals
(exit 0) & true & wait; printf '%s\n' $?
## stdout: |
0
## status: 0
#### job background-scope 0
## tags: posix, jobs-signals
v=0; (v=other) & wait; printf '%s\n' $v
## stdout: |
0
## status: 0
#### job exit-trap 0
## tags: posix, jobs-signals
trap 'printf "EXIT0\n"' 0; :
## stdout: |
EXIT0
## status: 0
#### job usr1-trap 0
## tags: posix, jobs-signals
trap 'printf "CAUGHT0\n"' USR1; kill -s USR1 $$; :
## stdout: |
CAUGHT0
## status: 0
#### job wait-status 1
## tags: posix, jobs-signals
(exit 1) & p=$!; wait $p; printf '%s\n' $?
## stdout: |
1
## status: 0
#### job wait-unknown 1
## tags: posix, jobs-signals
wait 900001 2>/dev/null; printf '%s\n' $?
## stdout: |
127
## status: 0
#### job wait-all 1
## tags: posix, jobs-signals
(exit 1) & true & wait; printf '%s\n' $?
## stdout: |
0
## status: 0
#### job background-scope 1
## tags: posix, jobs-signals
v=1; (v=other) & wait; printf '%s\n' $v
## stdout: |
1
## status: 0
#### job exit-trap 1
## tags: posix, jobs-signals
trap 'printf "EXIT1\n"' 0; :
## stdout: |
EXIT1
## status: 0
#### job usr1-trap 1
## tags: posix, jobs-signals
trap 'printf "CAUGHT1\n"' USR1; kill -s USR1 $$; :
## stdout: |
CAUGHT1
## status: 0
#### job wait-status 2
## tags: posix, jobs-signals
(exit 2) & p=$!; wait $p; printf '%s\n' $?
## stdout: |
2
## status: 0
#### job wait-unknown 2
## tags: posix, jobs-signals
wait 900002 2>/dev/null; printf '%s\n' $?
## stdout: |
127
## status: 0
#### job wait-all 2
## tags: posix, jobs-signals
(exit 2) & true & wait; printf '%s\n' $?
## stdout: |
0
## status: 0
#### job background-scope 2
## tags: posix, jobs-signals
v=2; (v=other) & wait; printf '%s\n' $v
## stdout: |
2
## status: 0
#### job exit-trap 2
## tags: posix, jobs-signals
trap 'printf "EXIT2\n"' 0; :
## stdout: |
EXIT2
## status: 0
#### job usr1-trap 2
## tags: posix, jobs-signals
trap 'printf "CAUGHT2\n"' USR1; kill -s USR1 $$; :
## stdout: |
CAUGHT2
## status: 0
#### job wait-status 3
## tags: posix, jobs-signals
(exit 3) & p=$!; wait $p; printf '%s\n' $?
## stdout: |
3
## status: 0
#### job wait-unknown 3
## tags: posix, jobs-signals
wait 900003 2>/dev/null; printf '%s\n' $?
## stdout: |
127
## status: 0
#### job wait-all 3
## tags: posix, jobs-signals
(exit 3) & true & wait; printf '%s\n' $?
## stdout: |
0
## status: 0
#### job background-scope 3
## tags: posix, jobs-signals
v=3; (v=other) & wait; printf '%s\n' $v
## stdout: |
3
## status: 0
#### job exit-trap 3
## tags: posix, jobs-signals
trap 'printf "EXIT3\n"' 0; :
## stdout: |
EXIT3
## status: 0
#### job usr1-trap 3
## tags: posix, jobs-signals
trap 'printf "CAUGHT3\n"' USR1; kill -s USR1 $$; :
## stdout: |
CAUGHT3
## status: 0
#### job wait-status 4
## tags: posix, jobs-signals
(exit 4) & p=$!; wait $p; printf '%s\n' $?
## stdout: |
4
## status: 0
#### job wait-unknown 4
## tags: posix, jobs-signals
wait 900004 2>/dev/null; printf '%s\n' $?
## stdout: |
127
## status: 0
#### job wait-all 4
## tags: posix, jobs-signals
(exit 4) & true & wait; printf '%s\n' $?
## stdout: |
0
## status: 0
#### job background-scope 4
## tags: posix, jobs-signals
v=4; (v=other) & wait; printf '%s\n' $v
## stdout: |
4
## status: 0
#### job exit-trap 4
## tags: posix, jobs-signals
trap 'printf "EXIT4\n"' 0; :
## stdout: |
EXIT4
## status: 0
#### job usr1-trap 4
## tags: posix, jobs-signals
trap 'printf "CAUGHT4\n"' USR1; kill -s USR1 $$; :
## stdout: |
CAUGHT4
## status: 0
