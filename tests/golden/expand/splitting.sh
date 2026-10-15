IFS=:; v=:a::b:; printf '<%s>\n' $v
v=::; printf '<%s>\n' $v
v=; printf '<%s>\n' $v "$v"
IFS=' :'; v=' : a :: b : '; printf '<%s>\n' $v
IFS=; v=' a  b '; printf '<%s>\n' $v
IFS=' '; v=' a   b '; printf '<%s>\n' x$v"y" "a b" a\ b
