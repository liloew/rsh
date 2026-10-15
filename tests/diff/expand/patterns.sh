# $- includes shell-specific flags; the f flag is asserted separately.
v=abcabc; printf '%s\n' "${v%c*}" "${v%%c*}" "${v#a*c}" "${v##a*c}" "${v%nomatch}"
v='a*b'; printf '%s\n' "${v%\*b}" "${v%'*b'}"
printf '<%s>\n' *.txt '?' \* [ab].txt [!a].txt [[:digit:]].txt .*.txt missing* "$v" $v
set -f; printf '%s\n' *.txt; set +f
