# Bad numeric fd: dash returns 2 for the failed command; bash and rsh return 1 (M4 requirement).
exec 3>f
exec 3>&-
echo bad >&3
printf '%s\n' "$?"
echo good
