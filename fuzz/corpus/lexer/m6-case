for x in a b '*' z; do case "$x" in a|b) printf 'ab\n';; '*') printf 'literal\n';; *) printf 'default\n';; esac; done
p='a*'; case abc in "$p") printf BAD;; $p) printf 'expanded\n';; esac
case x in (x) ;; esac; printf 'empty:%s\n' "$?"
case x in y) false;; esac; printf 'none:%s\n' "$?"
case Q in [[:upper:]]) printf 'class\n'
esac
