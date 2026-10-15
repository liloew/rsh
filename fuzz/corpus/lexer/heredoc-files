echo hi >f
cat <f
echo again >>f
cat f
echo forced >|f
cat f
exec 3<>rw
echo data >&3
exec 3<&-
cat rw
