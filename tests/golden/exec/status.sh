false
printf '%s\n' "$?"
false | true
printf '%s\n' "$?"
true | false
printf '%s\n' "$?"
! true
printf '%s\n' "$?"
! false
printf '%s\n' "$?"
false && true
printf '%s\n' "$?"
false
exit
