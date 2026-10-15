v='a b*'; a=$v; b=one c=$b; printf '<%s>\n' "$a" "$b" "$c"
f='file with *'; printf hi >$f; cat <"$f"; printf '\n'
HOME=/custom; v=~; p=~:/bin:~/child; printf '%s\n' "$v" "$p" ~ "~"
v=:; a=$v~; v=; b=$v~; printf '%s\n' "$a" "$b"
