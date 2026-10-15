v='a b'; export VALUE=$v; /usr/bin/printenv VALUE
export VALUE="quote'value"; export -p >listing
unset VALUE; . ./listing; printf '<%s>\n' "$VALUE"
