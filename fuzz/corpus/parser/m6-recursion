fact(){ if [ "$1" -le 1 ]; then printf 1; else printf '%s' "$(($1 * $(fact $(($1-1)))))"; fi; }
printf 'factorial:%s\n' "$(fact 6)"
f(){ case $1 in 0) return 3;; *) f $(($1-1));; esac; }; f 10; printf 'recursive-return:%s\n' "$?"
