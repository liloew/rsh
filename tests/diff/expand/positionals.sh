set --; set -- "$@"; printf '%s\n' "$#"; printf '<%s>\n' "$*"
set -- one; printf '<%s>\n' "$@" "$*"
set -- a '' 'b c'; printf '<%s>\n' "$@" "p$@s" "$*"
IFS=:; printf '<%s>\n' "$@" "$*"
IFS=; printf '<%s>\n' "$@" "$*" $@ $*
set -- 1 2 3 4 5 6 7 8 9 ten; printf '%s\n' "${10}" "$1" "$#" "$0"
