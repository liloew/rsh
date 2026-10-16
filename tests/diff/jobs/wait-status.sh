(exit 7) & p=$!
wait "$p"
echo status:$?
wait 999999 2>/dev/null
echo unknown:$?
(exit 3) & (exit 4) &
wait
echo all:$?
