# Optional dot arguments differ in reference shells; tested explicitly in builtins.rs.
set -- outer rest; PATH=.:/usr/bin:/bin; . source-file; printf 'status:%s:%s:%s\n' "$?" "$#" "$1"
