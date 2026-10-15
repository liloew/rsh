readonly FIXED='a b'; readonly -p >listing; test -s listing; echo $?; printf '<%s>\n' "$FIXED"
