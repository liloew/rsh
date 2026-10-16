cat &
p=$!
wait "$p"
echo eof:$?
printf 'redirected\n' > input
cat <input &
wait "$!"
