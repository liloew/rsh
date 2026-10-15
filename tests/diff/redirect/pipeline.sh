cat <<E | tr a-z A-Z
one
two
E
printf 'ignored\n' | cat <<E
replacement
E
