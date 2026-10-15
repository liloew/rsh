set -- a b c; shift; printf '%s:%s\n' "$#" "$1"; shift 0; shift 2; echo $#
