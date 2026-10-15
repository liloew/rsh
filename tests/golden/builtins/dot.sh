set -- outer rest
. ./source-file inner value; printf 'status:%s:%s:%s\n' "$?" "$#" "$1"
PATH=.:/usr/bin:/bin; . source-file; printf 'status:%s:%s:%s\n' "$?" "$#" "$1"
