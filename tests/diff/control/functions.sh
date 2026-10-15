set -- outer args
f(){ printf 'params:%s:<%s>:<%s>:%s\n' "$#" "$1" "$2" "$0"; shift; printf 'shift:%s:<%s>\n' "$#" "$1"; global=shared; return 7; printf BAD; }
f '' 'b c'; printf 'return:%s:%s:<%s>:%s\n' "$?" "$#" "$1" "$global"
f(){ false; return; }; f; printf 'default:%s\n' "$?"
printf(){ /usr/bin/printf 'function:%s\n' "$1"; }; printf overridden
true(){ /usr/bin/printf 'override-regular\n'; }; true
