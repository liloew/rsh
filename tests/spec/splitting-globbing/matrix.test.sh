# Original POSIX-derived cases. No Oils source copied.
# Reference: https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html
#### default IFS 0
## tags: posix, splitting-globbing
v=''; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
0
## status: 0
#### default IFS 1
## tags: posix, splitting-globbing
v='a'; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
1
<a>
## status: 0
#### default IFS 2
## tags: posix, splitting-globbing
v=' a '; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
1
<a>
## status: 0
#### default IFS 3
## tags: posix, splitting-globbing
v='a  b'; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
2
<a>
<b>
## status: 0
#### default IFS 4
## tags: posix, splitting-globbing
v='a	b
c'; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
3
<a>
<b>
<c>
## status: 0
#### default IFS 5
## tags: posix, splitting-globbing
v='   '; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
0
## status: 0
#### default IFS 6
## tags: posix, splitting-globbing
v='a:b'; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
1
<a:b>
## status: 0
#### default IFS 7
## tags: posix, splitting-globbing
v='a*'; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
1
<a*>
## status: 0
#### default IFS 8
## tags: posix, splitting-globbing
v='	a	'; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
1
<a>
## status: 0
#### default IFS 9
## tags: posix, splitting-globbing
v='a
b
'; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
2
<a>
<b>
## status: 0
#### nonwhite IFS 0
## tags: posix, splitting-globbing
IFS=:; v=''; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
0
## status: 0
#### nonwhite IFS 1
## tags: posix, splitting-globbing
IFS=:; v=':'; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
1
<>
## status: 0
#### nonwhite IFS 2
## tags: posix, splitting-globbing
IFS=:; v='::'; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
2
<>
<>
## status: 0
#### nonwhite IFS 3
## tags: posix, splitting-globbing
IFS=:; v=':a'; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
2
<>
<a>
## status: 0
#### nonwhite IFS 4
## tags: posix, splitting-globbing
IFS=:; v='a:'; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
1
<a>
## status: 0
#### nonwhite IFS 5
## tags: posix, splitting-globbing
IFS=:; v='a::b'; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
3
<a>
<>
<b>
## status: 0
#### nonwhite IFS 6
## tags: posix, splitting-globbing
IFS=:; v=':a:'; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
2
<>
<a>
## status: 0
#### nonwhite IFS 7
## tags: posix, splitting-globbing
IFS=:; v='a:b:c'; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
3
<a>
<b>
<c>
## status: 0
#### nonwhite IFS 8
## tags: posix, splitting-globbing
IFS=:; v='a::'; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
2
<a>
<>
## status: 0
#### nonwhite IFS 9
## tags: posix, splitting-globbing
IFS=:; v=' : '; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
2
< >
< >
## status: 0
#### IFS disabled 0
## tags: posix, splitting-globbing
set -f; IFS=; v=''; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
0
## status: 0
#### quoted empty retained 0
## tags: posix, splitting-globbing
v=''; set -- "$v"; printf '%s\n' "$#"; printf '<%s>\n' "$1"
## stdout: |
1
<>
## status: 0
#### IFS disabled 1
## tags: posix, splitting-globbing
set -f; IFS=; v='a b'; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
1
<a b>
## status: 0
#### quoted empty retained 1
## tags: posix, splitting-globbing
v='a b'; set -- "$v"; printf '%s\n' "$#"; printf '<%s>\n' "$1"
## stdout: |
1
<a b>
## status: 0
#### IFS disabled 2
## tags: posix, splitting-globbing
set -f; IFS=; v='a:b'; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
1
<a:b>
## status: 0
#### quoted empty retained 2
## tags: posix, splitting-globbing
v='a:b'; set -- "$v"; printf '%s\n' "$#"; printf '<%s>\n' "$1"
## stdout: |
1
<a:b>
## status: 0
#### IFS disabled 3
## tags: posix, splitting-globbing
set -f; IFS=; v='*'; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
1
<*>
## status: 0
#### quoted empty retained 3
## tags: posix, splitting-globbing
v='*'; set -- "$v"; printf '%s\n' "$#"; printf '<%s>\n' "$1"
## stdout: |
1
<*>
## status: 0
#### IFS disabled 4
## tags: posix, splitting-globbing
set -f; IFS=; v='
'; set -- $v; printf '%s\n' "$#"; for x; do printf '<%s>\n' "$x"; done
## stdout: |
1
<
>
## status: 0
#### quoted empty retained 4
## tags: posix, splitting-globbing
v='
'; set -- "$v"; printf '%s\n' "$#"; printf '<%s>\n' "$1"
## stdout: |
1
<
>
## status: 0
#### glob pattern 0
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; printf '<%s>\n' *.txt
## stdout: |
<a.txt>
<b.txt>
## status: 0
#### glob quoted pattern 0
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; printf '<%s>\n' '*.txt'
## stdout: |
<*.txt>
## status: 0
#### glob variable result 0
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; v='*.txt'; printf '<%s>\n' $v
## stdout: |
<a.txt>
<b.txt>
## status: 0
#### glob disabled pattern 0
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; set -f; printf '<%s>\n' *.txt
## stdout: |
<*.txt>
## status: 0
#### glob pattern 1
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; printf '<%s>\n' c?
## stdout: |
<c1>
<c2>
## status: 0
#### glob quoted pattern 1
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; printf '<%s>\n' 'c?'
## stdout: |
<c?>
## status: 0
#### glob variable result 1
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; v='c?'; printf '<%s>\n' $v
## stdout: |
<c1>
<c2>
## status: 0
#### glob disabled pattern 1
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; set -f; printf '<%s>\n' c?
## stdout: |
<c?>
## status: 0
#### glob pattern 2
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; printf '<%s>\n' [ab]*
## stdout: |
<a.txt>
<alpha>
<b.txt>
<beta>
## status: 0
#### glob quoted pattern 2
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; printf '<%s>\n' '[ab]*'
## stdout: |
<[ab]*>
## status: 0
#### glob variable result 2
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; v='[ab]*'; printf '<%s>\n' $v
## stdout: |
<a.txt>
<alpha>
<b.txt>
<beta>
## status: 0
#### glob disabled pattern 2
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; set -f; printf '<%s>\n' [ab]*
## stdout: |
<[ab]*>
## status: 0
#### glob pattern 3
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; printf '<%s>\n' [!a.]*
## stdout: |
<b.txt>
<beta>
<c1>
<c2>
## status: 0
#### glob quoted pattern 3
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; printf '<%s>\n' '[!a.]*'
## stdout: |
<[!a.]*>
## status: 0
#### glob variable result 3
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; v='[!a.]*'; printf '<%s>\n' $v
## stdout: |
<b.txt>
<beta>
<c1>
<c2>
## status: 0
#### glob disabled pattern 3
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; set -f; printf '<%s>\n' [!a.]*
## stdout: |
<[!a.]*>
## status: 0
#### glob pattern 4
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; printf '<%s>\n' .d*
## stdout: |
<.dot>
## status: 0
#### glob quoted pattern 4
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; printf '<%s>\n' '.d*'
## stdout: |
<.d*>
## status: 0
#### glob variable result 4
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; v='.d*'; printf '<%s>\n' $v
## stdout: |
<.dot>
## status: 0
#### glob disabled pattern 4
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; set -f; printf '<%s>\n' .d*
## stdout: |
<.d*>
## status: 0
#### glob pattern 5
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; printf '<%s>\n' missing*
## stdout: |
<missing*>
## status: 0
#### glob quoted pattern 5
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; printf '<%s>\n' 'missing*'
## stdout: |
<missing*>
## status: 0
#### glob variable result 5
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; v='missing*'; printf '<%s>\n' $v
## stdout: |
<missing*>
## status: 0
#### glob disabled pattern 5
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; set -f; printf '<%s>\n' missing*
## stdout: |
<missing*>
## status: 0
#### glob pattern 6
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; printf '<%s>\n' [[:digit:]]*
## stdout: |
<[[:digit:]]*>
## status: 0
#### glob quoted pattern 6
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; printf '<%s>\n' '[[:digit:]]*'
## stdout: |
<[[:digit:]]*>
## status: 0
#### glob variable result 6
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; v='[[:digit:]]*'; printf '<%s>\n' $v
## stdout: |
<[[:digit:]]*>
## status: 0
#### glob disabled pattern 6
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; set -f; printf '<%s>\n' [[:digit:]]*
## stdout: |
<[[:digit:]]*>
## status: 0
#### glob pattern 7
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; printf '<%s>\n' c[12]
## stdout: |
<c1>
<c2>
## status: 0
#### glob quoted pattern 7
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; printf '<%s>\n' 'c[12]'
## stdout: |
<c[12]>
## status: 0
#### glob variable result 7
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; v='c[12]'; printf '<%s>\n' $v
## stdout: |
<c1>
<c2>
## status: 0
#### glob disabled pattern 7
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; set -f; printf '<%s>\n' c[12]
## stdout: |
<c[12]>
## status: 0
#### glob pattern 8
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; printf '<%s>\n' a*
## stdout: |
<a.txt>
<alpha>
## status: 0
#### glob quoted pattern 8
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; printf '<%s>\n' 'a*'
## stdout: |
<a*>
## status: 0
#### glob variable result 8
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; v='a*'; printf '<%s>\n' $v
## stdout: |
<a.txt>
<alpha>
## status: 0
#### glob disabled pattern 8
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; set -f; printf '<%s>\n' a*
## stdout: |
<a*>
## status: 0
#### glob pattern 9
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; printf '<%s>\n' b*
## stdout: |
<b.txt>
<beta>
## status: 0
#### glob quoted pattern 9
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; printf '<%s>\n' 'b*'
## stdout: |
<b*>
## status: 0
#### glob variable result 9
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; v='b*'; printf '<%s>\n' $v
## stdout: |
<b.txt>
<beta>
## status: 0
#### glob disabled pattern 9
## tags: posix, splitting-globbing
mkdir patterns; cd patterns; printf x >'a.txt'; printf x >'b.txt'; printf x >'c1'; printf x >'c2'; printf x >'.dot'; printf x >'alpha'; printf x >'beta'; set -f; printf '<%s>\n' b*
## stdout: |
<b*>
## status: 0
