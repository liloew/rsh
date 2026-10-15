a && b || c ; d &
! a | b
A=1 >out B=2 cmd C=3
2>file
2 >file
{ a; }
(a)
if a; then b; elif c; then d; else e; fi
if (false) then (echo x) else (echo y) fi
while a; do b; done
until a; do b; done
for i; do :; done
for i do :; done
for i in; do :; done
for i in if then do; do echo if; done
case x in (a|b) ;; esac
case x in a|esac) ;; esac
case x in (esac) ;; esac
case x in a) esac
case x in esac
echo if then do done fi
'if' then
A=1 if then
>out if
if (:) then { (:); } else (:) fi
