# POSIX 2.10.2 requires an unquoted NAME; bash -n defers this check until execution.
# POSIX requires a WORD before a case pattern pipe; dash accepts an empty first pattern.
&& a
a | | b
! ! a
a | ! b
if ; then :; fi
if a; then ; fi
while a; then b; done
until a; do ; done
for 1; do :; done
for 'i'; do :; done
for i; in a; do :; done
case x in |) ;; esac
case x in esac|a) ;; esac
{ ; }
()
a ;; b
a & & b
echo > ;
