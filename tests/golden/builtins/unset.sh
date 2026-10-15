v=x; unset -v v; printf '<%s>\n' "${v-unset}"
f(){ :; }; unset -f f; command -v f >/dev/null; test $? -ne 0; echo $?
