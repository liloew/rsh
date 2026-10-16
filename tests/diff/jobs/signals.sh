sleep 100 & p=$!
kill -s TERM "$p"
wait "$p" 2>/dev/null
echo killed:$?
kill -l 15
