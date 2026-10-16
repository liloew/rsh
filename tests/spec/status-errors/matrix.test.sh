# Original POSIX-derived cases. No Oils source copied.
# Reference: https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html
#### status exit 0
## tags: posix, status-errors
exit 0
## stdout: |
## status: 0
#### status last 0
## tags: posix, status-errors
false; (exit 0)
## stdout: |
## status: 0
#### status pipeline-last 0
## tags: posix, status-errors
(exit 0) | true
## stdout: |
## status: 0
#### status pipeline-fail 0
## tags: posix, status-errors
true | (exit 0)
## stdout: |
## status: 0
#### status negate 0
## tags: posix, status-errors
! (exit 0)
## stdout: |
## status: 1
#### status missing 0
## tags: posix, status-errors
no_such_rsh_m10_command_0
## stdout: |
## status: 127
#### status permission 0
## tags: posix, status-errors
printf x >not-executable-0; ./not-executable-0
## stdout: |
## status: 126
#### status script-fallback 0
## tags: posix, status-errors
printf '%s\n' 'printf '\''FALLBACK0\n'\''' >script; chmod +x script; ./script
## stdout: |
FALLBACK0
## status: 0
#### status exit 1
## tags: posix, status-errors
exit 1
## stdout: |
## status: 1
#### status last 1
## tags: posix, status-errors
false; (exit 1)
## stdout: |
## status: 1
#### status pipeline-last 1
## tags: posix, status-errors
(exit 1) | true
## stdout: |
## status: 0
#### status pipeline-fail 1
## tags: posix, status-errors
true | (exit 1)
## stdout: |
## status: 1
#### status negate 1
## tags: posix, status-errors
! (exit 1)
## stdout: |
## status: 0
#### status missing 1
## tags: posix, status-errors
no_such_rsh_m10_command_1
## stdout: |
## status: 127
#### status permission 1
## tags: posix, status-errors
printf x >not-executable-1; ./not-executable-1
## stdout: |
## status: 126
#### status script-fallback 1
## tags: posix, status-errors
printf '%s\n' 'printf '\''FALLBACK1\n'\''' >script; chmod +x script; ./script
## stdout: |
FALLBACK1
## status: 0
#### status exit 2
## tags: posix, status-errors
exit 2
## stdout: |
## status: 2
#### status last 2
## tags: posix, status-errors
false; (exit 2)
## stdout: |
## status: 2
#### status pipeline-last 2
## tags: posix, status-errors
(exit 2) | true
## stdout: |
## status: 0
#### status pipeline-fail 2
## tags: posix, status-errors
true | (exit 2)
## stdout: |
## status: 2
#### status negate 2
## tags: posix, status-errors
! (exit 2)
## stdout: |
## status: 0
#### status missing 2
## tags: posix, status-errors
no_such_rsh_m10_command_2
## stdout: |
## status: 127
#### status permission 2
## tags: posix, status-errors
printf x >not-executable-2; ./not-executable-2
## stdout: |
## status: 126
#### status script-fallback 2
## tags: posix, status-errors
printf '%s\n' 'printf '\''FALLBACK2\n'\''' >script; chmod +x script; ./script
## stdout: |
FALLBACK2
## status: 0
#### status exit 3
## tags: posix, status-errors
exit 3
## stdout: |
## status: 3
#### status last 3
## tags: posix, status-errors
false; (exit 3)
## stdout: |
## status: 3
#### status pipeline-last 3
## tags: posix, status-errors
(exit 3) | true
## stdout: |
## status: 0
#### status pipeline-fail 3
## tags: posix, status-errors
true | (exit 3)
## stdout: |
## status: 3
#### status negate 3
## tags: posix, status-errors
! (exit 3)
## stdout: |
## status: 0
#### status missing 3
## tags: posix, status-errors
no_such_rsh_m10_command_3
## stdout: |
## status: 127
#### status permission 3
## tags: posix, status-errors
printf x >not-executable-3; ./not-executable-3
## stdout: |
## status: 126
#### status script-fallback 3
## tags: posix, status-errors
printf '%s\n' 'printf '\''FALLBACK3\n'\''' >script; chmod +x script; ./script
## stdout: |
FALLBACK3
## status: 0
#### status exit 4
## tags: posix, status-errors
exit 4
## stdout: |
## status: 4
#### status last 4
## tags: posix, status-errors
false; (exit 4)
## stdout: |
## status: 4
#### status pipeline-last 4
## tags: posix, status-errors
(exit 4) | true
## stdout: |
## status: 0
#### status pipeline-fail 4
## tags: posix, status-errors
true | (exit 4)
## stdout: |
## status: 4
#### status negate 4
## tags: posix, status-errors
! (exit 4)
## stdout: |
## status: 0
#### status missing 4
## tags: posix, status-errors
no_such_rsh_m10_command_4
## stdout: |
## status: 127
#### status permission 4
## tags: posix, status-errors
printf x >not-executable-4; ./not-executable-4
## stdout: |
## status: 126
#### status script-fallback 4
## tags: posix, status-errors
printf '%s\n' 'printf '\''FALLBACK4\n'\''' >script; chmod +x script; ./script
## stdout: |
FALLBACK4
## status: 0
