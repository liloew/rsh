set -- a '' 'b c'; for x; do printf '<%s>\n' "$x"; done; printf 'last:<%s>\n' "$x"
v='d e'; for x in $v "$v"; do printf '<%s>\n' "$x"; done
for x in a b; do set -- changed; printf '%s\n' "$x"; done
for x in a b; do false; done; printf 'status:%s\n' "$?"
