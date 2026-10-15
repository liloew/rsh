eval "v='a b'"; printf '<%s>\n' "$v"
eval 'printf' "'%s\n'" "'quoted value'"
f(){ eval 'return 7'; printf BAD; }; f; echo $?
eval; echo $?
