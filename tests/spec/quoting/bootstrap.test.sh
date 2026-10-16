# Original tests for POSIX.1-2024 2.2; no external test suite copied.
#### quoting bootstrap literal
## tags: posix, quoting
printf '%s\n' 'a b' "c d" e\ f
## stdout: |
a b
c d
e f
## status: 0
#### quoting bootstrap empty
## tags: posix, quoting
set -- '' ""; printf '%s:<%s>:<%s>\n' "$#" "$1" "$2"
## stdout: |
2:<>:<>
## status: 0
