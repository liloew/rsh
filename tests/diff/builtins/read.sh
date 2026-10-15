printf ' a  b c \n' | { read a b; printf '<%s><%s>\n' "$a" "$b"; }
printf ':a::b:c:\n' | { IFS=: read a b c; printf '<%s><%s><%s>\n' "$a" "$b" "$c"; }
printf '%s\n' 'a\ b' 'raw\line' | { read a; read -r b; printf '<%s><%s>\n' "$a" "$b"; }
printf 'a\\\nb\n' | { read x; echo "$x"; }
printf partial | { read x; printf '%s:<%s>\n' "$?" "$x"; }
printf 'a\nb\n' | { read x; cat; echo "$x"; }
