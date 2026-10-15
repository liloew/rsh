set -e
if false; then echo BAD; fi
while false; do echo BAD; done
until true; do echo BAD; done
false && echo BAD
false || echo recovered
! true
f(){ false; echo function-continues; }; if f; then echo condition-true; fi
set -u; set --; printf 'args:%s:<%s>\n' "$#" "$*"; printf '<%s>\n' "${missing-default}"
