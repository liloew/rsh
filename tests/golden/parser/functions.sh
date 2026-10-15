f() { return "$1"; } >"$2"
g() if true; then f value; fi
