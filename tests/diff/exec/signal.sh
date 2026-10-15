# bash may exec its final external command and itself terminate by SIGTERM.
# Compare numeric 128+signal for both wait-status forms; ignore signal notices.
./term
