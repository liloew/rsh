# Original POSIX-derived cases. No Oils source copied.
# Reference: https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html
#### syntax invalid 0
## tags: posix, syntax
)
## stdout: |
## status: 2
#### syntax invalid 1
## tags: posix, syntax
&& :
## stdout: |
## status: 2
#### syntax invalid 2
## tags: posix, syntax
: |
## stdout: |
## status: 2
#### syntax invalid 3
## tags: posix, syntax
if true; then
## stdout: |
## status: 2
#### syntax invalid 4
## tags: posix, syntax
while :; do
## stdout: |
## status: 2
#### syntax invalid 5
## tags: posix, syntax
for
## stdout: |
## status: 2
#### syntax invalid 6
## tags: posix, syntax
case x in x)
## stdout: |
## status: 2
#### syntax invalid 7
## tags: posix, syntax
f()
## stdout: |
## status: 2
#### syntax invalid 8
## tags: posix, syntax
{ :;
## stdout: |
## status: 2
#### syntax invalid 9
## tags: posix, syntax
( :
## stdout: |
## status: 2
#### syntax invalid 10
## tags: posix, syntax
printf 'open
## stdout: |
## status: 2
#### syntax invalid 11
## tags: posix, syntax
printf "open
## stdout: |
## status: 2
#### syntax invalid 12
## tags: posix, syntax
printf ${v
## stdout: |
## status: 2
#### syntax invalid 13
## tags: posix, syntax
printf $(printf x
## stdout: |
## status: 2
#### syntax invalid 14
## tags: posix, syntax
printf `printf x
## stdout: |
## status: 2
#### syntax valid 0
## tags: posix, syntax
{ (:); }
## stdout: |
## status: 0
#### syntax valid 1
## tags: posix, syntax
{ (:) }
## stdout: |
## status: 0
#### syntax valid 2
## tags: posix, syntax
if (:) then :; fi
## stdout: |
## status: 0
#### syntax valid 3
## tags: posix, syntax
case x in esac
## stdout: |
## status: 0
#### syntax valid 4
## tags: posix, syntax
case x in (a|b) ;; esac
## stdout: |
## status: 0
#### syntax valid 5
## tags: posix, syntax
for i; do :; done
## stdout: |
## status: 0
#### syntax valid 6
## tags: posix, syntax
! true | false
## stdout: |
## status: 0
#### syntax valid 7
## tags: posix, syntax
true &&
 true
## stdout: |
## status: 0
#### syntax valid 8
## tags: posix, syntax
true |
 true
## stdout: |
## status: 0
#### syntax valid 9
## tags: posix, syntax
if true; then
 :; fi
## stdout: |
## status: 0
#### syntax valid 10
## tags: posix, syntax
while false; do
 :; done
## stdout: |
## status: 0
#### syntax valid 11
## tags: posix, syntax
case x in x) :; esac
## stdout: |
## status: 0
#### syntax valid 12
## tags: posix, syntax
f()
{ :; }; f
## stdout: |
## status: 0
#### syntax valid 13
## tags: posix, syntax
case x in x)
;; esac
## stdout: |
## status: 0
#### syntax valid 14
## tags: posix, syntax
for i in; do :; done
## stdout: |
## status: 0
