set -- a 'b c'; printf '%s:<%s>\n' "$#" "$2"
set -fC; case $- in *f*C*|*C*f*) echo flags;; esac; set +fC
set +o >options; set -o >human; test -s human; echo $?; . ./options
set >variables; test -s variables; echo $?
