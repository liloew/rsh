alias thing='echo hello'; alias >aliases
unalias thing; eval "alias $(cat aliases)"
alias thing
