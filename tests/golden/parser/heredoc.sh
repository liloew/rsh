cat <<A <<-'B'
$x ) ' "
A
	literal
B
echo "$(cat <<E
) data
E
)"
