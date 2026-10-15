{ echo one; echo two; } >f
cat f
(cat) <f
X=old
{ X=brace; } >g
printf '%s\n' "$X"
(X=subshell; echo isolated) >g
printf '%s\n' "$X"
cat g
{ cat <<E; echo done; } >h
here
E
cat h
