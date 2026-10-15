x=parent; (x=child; cd child; printf 'child:%s\n' "$x"); printf 'parent:%s\n' "$x"; pwd
{ x=brace; cd child; }; printf 'brace:%s\n' "$x"; pwd; cd ..
(exit 4); printf 'subexit:%s\n' "$?"
f(){ (exit 5); printf 'function-survives:%s\n' "$?"; }; f
{ printf 'pipeline\n'; x=pipeline; } | cat; printf 'parent:%s\n' "$x"
printf data | while true; do cat; break; done
