# Original regression cases: POSIX 2.9.1.6 text-script fallback.
#### fallback retains positional arguments
printf '%s\n' 'printf "<%s>\n" "$@"' >script
chmod +x script
./script 'a b' '' c
## stdout: |
<a b>
<>
<c>
## status: 0
#### exec text script replaces caller
printf '%s\n' 'printf "executed\n"; exit 7' >script
chmod +x script
exec ./script
echo unreachable
## stdout: |
executed
## status: 7
#### fallback follows PATH and preserves status
printf '%s\n' 'printf "path\n"; exit 9' >script
chmod +x script
PATH=.:/bin:/usr/bin script
## stdout: |
path
## status: 9
