(exit 7) & p=$!; wait "$p"; printf 'status:%s\n' "$?"
true & false & wait; echo $?
