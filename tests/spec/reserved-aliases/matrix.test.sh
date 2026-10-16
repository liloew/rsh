# Original POSIX-derived cases. No Oils source copied.
# Reference: https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html
#### reserved argument if
## tags: posix, reserved-aliases
printf '<%s>\n' if
## stdout: |
<if>
## status: 0
#### reserved argument then
## tags: posix, reserved-aliases
printf '<%s>\n' then
## stdout: |
<then>
## status: 0
#### reserved argument else
## tags: posix, reserved-aliases
printf '<%s>\n' else
## stdout: |
<else>
## status: 0
#### reserved argument fi
## tags: posix, reserved-aliases
printf '<%s>\n' fi
## stdout: |
<fi>
## status: 0
#### reserved argument while
## tags: posix, reserved-aliases
printf '<%s>\n' while
## stdout: |
<while>
## status: 0
#### reserved argument do
## tags: posix, reserved-aliases
printf '<%s>\n' do
## stdout: |
<do>
## status: 0
#### reserved argument done
## tags: posix, reserved-aliases
printf '<%s>\n' done
## stdout: |
<done>
## status: 0
#### reserved argument case
## tags: posix, reserved-aliases
printf '<%s>\n' case
## stdout: |
<case>
## status: 0
#### reserved argument esac
## tags: posix, reserved-aliases
printf '<%s>\n' esac
## stdout: |
<esac>
## status: 0
#### reserved argument in
## tags: posix, reserved-aliases
printf '<%s>\n' in
## stdout: |
<in>
## status: 0
#### alias invocation 0
## tags: posix, reserved-aliases
alias rsh_m10_aa='printf ALIAS0'
rsh_m10_aa
printf '\n'
## stdout: |
ALIAS0
## status: 0
#### alias trailing blank 0
## tags: posix, reserved-aliases
alias rsh_m10_aa='printf "%s\n" ' rsh_m10_bb='VALUE0'
rsh_m10_aa rsh_m10_bb
## stdout: |
VALUE0
## status: 0
#### unalias 0
## tags: posix, reserved-aliases
alias rsh_m10_aa='printf 0'
unalias rsh_m10_aa
if command -v rsh_m10_aa >/dev/null; then printf 'found\n'; else printf 'absent\n'; fi
## stdout: |
absent
## status: 0
#### quoted command bypasses alias 0
## tags: posix, reserved-aliases
alias printf='false'
'printf' '%s\n' 0
## stdout: |
0
## status: 0
#### alias invocation 1
## tags: posix, reserved-aliases
alias rsh_m10_aa='printf ALIAS1'
rsh_m10_aa
printf '\n'
## stdout: |
ALIAS1
## status: 0
#### alias trailing blank 1
## tags: posix, reserved-aliases
alias rsh_m10_aa='printf "%s\n" ' rsh_m10_bb='VALUE1'
rsh_m10_aa rsh_m10_bb
## stdout: |
VALUE1
## status: 0
#### unalias 1
## tags: posix, reserved-aliases
alias rsh_m10_aa='printf 1'
unalias rsh_m10_aa
if command -v rsh_m10_aa >/dev/null; then printf 'found\n'; else printf 'absent\n'; fi
## stdout: |
absent
## status: 0
#### quoted command bypasses alias 1
## tags: posix, reserved-aliases
alias printf='false'
'printf' '%s\n' 1
## stdout: |
1
## status: 0
#### alias invocation 2
## tags: posix, reserved-aliases
alias rsh_m10_aa='printf ALIAS2'
rsh_m10_aa
printf '\n'
## stdout: |
ALIAS2
## status: 0
#### alias trailing blank 2
## tags: posix, reserved-aliases
alias rsh_m10_aa='printf "%s\n" ' rsh_m10_bb='VALUE2'
rsh_m10_aa rsh_m10_bb
## stdout: |
VALUE2
## status: 0
#### unalias 2
## tags: posix, reserved-aliases
alias rsh_m10_aa='printf 2'
unalias rsh_m10_aa
if command -v rsh_m10_aa >/dev/null; then printf 'found\n'; else printf 'absent\n'; fi
## stdout: |
absent
## status: 0
#### quoted command bypasses alias 2
## tags: posix, reserved-aliases
alias printf='false'
'printf' '%s\n' 2
## stdout: |
2
## status: 0
#### alias invocation 3
## tags: posix, reserved-aliases
alias rsh_m10_aa='printf ALIAS3'
rsh_m10_aa
printf '\n'
## stdout: |
ALIAS3
## status: 0
#### alias trailing blank 3
## tags: posix, reserved-aliases
alias rsh_m10_aa='printf "%s\n" ' rsh_m10_bb='VALUE3'
rsh_m10_aa rsh_m10_bb
## stdout: |
VALUE3
## status: 0
#### unalias 3
## tags: posix, reserved-aliases
alias rsh_m10_aa='printf 3'
unalias rsh_m10_aa
if command -v rsh_m10_aa >/dev/null; then printf 'found\n'; else printf 'absent\n'; fi
## stdout: |
absent
## status: 0
#### quoted command bypasses alias 3
## tags: posix, reserved-aliases
alias printf='false'
'printf' '%s\n' 3
## stdout: |
3
## status: 0
#### alias invocation 4
## tags: posix, reserved-aliases
alias rsh_m10_aa='printf ALIAS4'
rsh_m10_aa
printf '\n'
## stdout: |
ALIAS4
## status: 0
#### alias trailing blank 4
## tags: posix, reserved-aliases
alias rsh_m10_aa='printf "%s\n" ' rsh_m10_bb='VALUE4'
rsh_m10_aa rsh_m10_bb
## stdout: |
VALUE4
## status: 0
#### unalias 4
## tags: posix, reserved-aliases
alias rsh_m10_aa='printf 4'
unalias rsh_m10_aa
if command -v rsh_m10_aa >/dev/null; then printf 'found\n'; else printf 'absent\n'; fi
## stdout: |
absent
## status: 0
#### quoted command bypasses alias 4
## tags: posix, reserved-aliases
alias printf='false'
'printf' '%s\n' 4
## stdout: |
4
## status: 0
