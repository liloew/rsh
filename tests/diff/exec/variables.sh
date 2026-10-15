A=local
printf '%s\n' "$A"
A=temp printenv A
printf '%s\n' "$A"
printenv A || true
export A
printenv A
A=next
printenv A
A=scoped true
printf '%s\n' "$A"
A=persist :
printf '%s\n' "$A"
export B=hello
printenv B
