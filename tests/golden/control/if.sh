if false; then printf BAD; elif true; then printf 'elif\n'; else printf BAD; fi
if false; then :; fi; printf 'none:%s\n' "$?"
if false | true; then printf 'pipe\n'; fi
if ! false; then printf 'negated\n'; fi
if true; then false; else true; fi; printf 'body:%s\n' "$?"
