printf '%s:%s;' a b c; printf '\n'
printf '%d %i %o %u %x %X %c %.3s %%\n' -3 0x10 8 9 31 31 Z abcdef
printf '[%+06d][%#08x][%-5s][%*.*s]\n' 3 31 x 5 2 abc
printf '%bEND\n' 'a\tb'; printf 'missing:%d:<%s>\n'
printf '%bBAD' 'stop\c'; printf '\n'
