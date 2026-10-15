exec 3>fd-file; echo written >&3; exec 3>&-; cat fd-file
exec /usr/bin/printf 'replaced\n'
echo BAD
