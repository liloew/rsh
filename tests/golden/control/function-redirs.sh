f(){ printf '%s\n' "$1"; } >"$1"
f first; f second; cat first second
f third >call; cat third; cat call; printf 'empty\n'
f(){ cat; } <<END
hello $1
END
f one; f two
f(){ printf before; { return 6; }; printf BAD; } >out
f; printf 'status:%s\n' "$?"; cat out; printf '\nrestored\n'
