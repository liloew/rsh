trap 'echo exit:$?' EXIT
trap '' USR1
trap
false
