for x in a b; do for y in 1 2; do printf '%s%s\n' "$x" "$y"; break 2; done; printf BAD; done
for x in a b; do for y in 1 2; do printf '%s%s\n' "$x" "$y"; continue 2; done; printf BAD; done
for x in a b; do { if true; then break 99; fi; }; printf BAD; done
break; continue; printf 'outside:%s\n' "$?"
i=0; while i=$((i+1)); [ "$i" -lt 3 ]; do continue; done; printf 'i:%s\n' "$i"
