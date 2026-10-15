f() if true; then printf 'if-body\n'; fi
f
f() for x in a b; do printf '%s\n' "$x"; done
f
f() (x=subshell; printf '%s\n' "$1")
x=parent; f isolated; printf '%s\n' "$x"
