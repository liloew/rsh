# dash preserves the unexported flag; bash --posix exports A after this special builtin.
# rsh follows dash: assignment persists, but only explicit export sets a new export flag.
A=1 :
printf '%s\n' "$A"
env | grep '^A='
