pwd >created 2>errors <missing
printf '%s\n' "$?"
echo stdout-restored
./both
cat errors
