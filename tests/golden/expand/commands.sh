v=$(printf 'a\n\nb\n\n'); printf '<%s>\n' "$v"
v=$(false); printf '%s\n' "$?"
v=$(true; false); printf '%s\n' "$?"
printf '<%s>\n' "$(printf '%s' "$(printf hi)")" `printf backtick`
printf '%s\n' "`printf '%s' \"\$HOME\"`"
v=parent; printf '%s\n' "$(v=child; printf '%s' "$v")" "$v"
printf '%s\n' "$(cat <<'END'
) quote " raw
END
)"
