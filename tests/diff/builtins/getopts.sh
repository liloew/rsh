set -- -a -b value rest
while getopts 'ab:' opt; do printf '%s:<%s>:%s\n' "$opt" "${OPTARG-}" "$OPTIND"; done
OPTIND=1; getopts ':a:' opt -a; printf '%s:%s:%s\n' "$opt" "$OPTARG" "$OPTIND"
OPTIND=1; getopts ':a' opt -z; printf '%s:%s:%s\n' "$opt" "$OPTARG" "$OPTIND"
