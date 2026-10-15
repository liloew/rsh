f(){ printf '%s:%s\n' "$1" "$#"; }; f pipe | cat
printf data | f sink
( f subshell ); printf 'capture:<%s>\n' "$(f capture)"
f(){ if true; then for x in a b; do return 9; done; fi; printf BAD; }; f; printf 'nested-return:%s\n' "$?"
f(){ f(){ printf 'new\n'; }; printf 'old\n'; }; f; f
