x='a b*'; cat <<END
$x ${v:-default} $((2+3)) $(printf 'sub\n\n') "literal" \$x
END
cat <<'END'
$x $(false) $((1/0))
END
