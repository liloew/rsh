# Original POSIX-derived cases. No Oils source copied.
# Reference: https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html
#### quoting single value 0
## tags: posix, quoting
printf '<%s>\n' ''
## stdout: |
<>
## status: 0
#### quoting double value 0
## tags: posix, quoting
x=''; printf '<%s>\n' "$x"
## stdout: |
<>
## status: 0
#### quoting concatenate value 0
## tags: posix, quoting
x=''; printf '<%s>\n' pre"$x"post
## stdout: |
<prepost>
## status: 0
#### quoting literal-dollar value 0
## tags: posix, quoting
x=''; printf '%s\n' '$x'
## stdout: |
$x
## status: 0
#### quoting escaped-space value 0
## tags: posix, quoting
printf '%s\n' a\ b ''
## stdout: |
a b

## status: 0
#### quoting comment-boundary value 0
## tags: posix, quoting
printf '%s\n' a#b '' # ignored | ;
## stdout: |
a#b

## status: 0
#### quoting quoted-operators value 0
## tags: posix, quoting
printf '%s\n' '&& || ;; < >' ''
## stdout: |
&& || ;; < >

## status: 0
#### quoting empty-adjacent value 0
## tags: posix, quoting
printf '<%s>\n' ''''""
## stdout: |
<>
## status: 0
#### quoting continuation value 0
## tags: posix, quoting
printf '%s\n' ab\
cd ''
## stdout: |
abcd

## status: 0
#### quoting escaped-backslash value 0
## tags: posix, quoting
printf '%s\n' \\ ''
## stdout: |
\

## status: 0
#### quoting single value 1
## tags: posix, quoting
printf '<%s>\n' 'a b'
## stdout: |
<a b>
## status: 0
#### quoting double value 1
## tags: posix, quoting
x='a b'; printf '<%s>\n' "$x"
## stdout: |
<a b>
## status: 0
#### quoting concatenate value 1
## tags: posix, quoting
x='a b'; printf '<%s>\n' pre"$x"post
## stdout: |
<prea bpost>
## status: 0
#### quoting literal-dollar value 1
## tags: posix, quoting
x='a b'; printf '%s\n' '$x'
## stdout: |
$x
## status: 0
#### quoting escaped-space value 1
## tags: posix, quoting
printf '%s\n' a\ b 'a b'
## stdout: |
a b
a b
## status: 0
#### quoting comment-boundary value 1
## tags: posix, quoting
printf '%s\n' a#b 'a b' # ignored | ;
## stdout: |
a#b
a b
## status: 0
#### quoting quoted-operators value 1
## tags: posix, quoting
printf '%s\n' '&& || ;; < >' 'a b'
## stdout: |
&& || ;; < >
a b
## status: 0
#### quoting empty-adjacent value 1
## tags: posix, quoting
printf '<%s>\n' '''a b'""
## stdout: |
<a b>
## status: 0
#### quoting continuation value 1
## tags: posix, quoting
printf '%s\n' ab\
cd 'a b'
## stdout: |
abcd
a b
## status: 0
#### quoting escaped-backslash value 1
## tags: posix, quoting
printf '%s\n' \\ 'a b'
## stdout: |
\
a b
## status: 0
#### quoting single value 2
## tags: posix, quoting
printf '<%s>\n' '*?[ab]'
## stdout: |
<*?[ab]>
## status: 0
#### quoting double value 2
## tags: posix, quoting
x='*?[ab]'; printf '<%s>\n' "$x"
## stdout: |
<*?[ab]>
## status: 0
#### quoting concatenate value 2
## tags: posix, quoting
x='*?[ab]'; printf '<%s>\n' pre"$x"post
## stdout: |
<pre*?[ab]post>
## status: 0
#### quoting literal-dollar value 2
## tags: posix, quoting
x='*?[ab]'; printf '%s\n' '$x'
## stdout: |
$x
## status: 0
#### quoting escaped-space value 2
## tags: posix, quoting
printf '%s\n' a\ b '*?[ab]'
## stdout: |
a b
*?[ab]
## status: 0
#### quoting comment-boundary value 2
## tags: posix, quoting
printf '%s\n' a#b '*?[ab]' # ignored | ;
## stdout: |
a#b
*?[ab]
## status: 0
#### quoting quoted-operators value 2
## tags: posix, quoting
printf '%s\n' '&& || ;; < >' '*?[ab]'
## stdout: |
&& || ;; < >
*?[ab]
## status: 0
#### quoting empty-adjacent value 2
## tags: posix, quoting
printf '<%s>\n' '''*?[ab]'""
## stdout: |
<*?[ab]>
## status: 0
#### quoting continuation value 2
## tags: posix, quoting
printf '%s\n' ab\
cd '*?[ab]'
## stdout: |
abcd
*?[ab]
## status: 0
#### quoting escaped-backslash value 2
## tags: posix, quoting
printf '%s\n' \\ '*?[ab]'
## stdout: |
\
*?[ab]
## status: 0
#### quoting single value 3
## tags: posix, quoting
printf '<%s>\n' '$x;#&|'
## stdout: |
<$x;#&|>
## status: 0
#### quoting double value 3
## tags: posix, quoting
x='$x;#&|'; printf '<%s>\n' "$x"
## stdout: |
<$x;#&|>
## status: 0
#### quoting concatenate value 3
## tags: posix, quoting
x='$x;#&|'; printf '<%s>\n' pre"$x"post
## stdout: |
<pre$x;#&|post>
## status: 0
#### quoting literal-dollar value 3
## tags: posix, quoting
x='$x;#&|'; printf '%s\n' '$x'
## stdout: |
$x
## status: 0
#### quoting escaped-space value 3
## tags: posix, quoting
printf '%s\n' a\ b '$x;#&|'
## stdout: |
a b
$x;#&|
## status: 0
#### quoting comment-boundary value 3
## tags: posix, quoting
printf '%s\n' a#b '$x;#&|' # ignored | ;
## stdout: |
a#b
$x;#&|
## status: 0
#### quoting quoted-operators value 3
## tags: posix, quoting
printf '%s\n' '&& || ;; < >' '$x;#&|'
## stdout: |
&& || ;; < >
$x;#&|
## status: 0
#### quoting empty-adjacent value 3
## tags: posix, quoting
printf '<%s>\n' '''$x;#&|'""
## stdout: |
<$x;#&|>
## status: 0
#### quoting continuation value 3
## tags: posix, quoting
printf '%s\n' ab\
cd '$x;#&|'
## stdout: |
abcd
$x;#&|
## status: 0
#### quoting escaped-backslash value 3
## tags: posix, quoting
printf '%s\n' \\ '$x;#&|'
## stdout: |
\
$x;#&|
## status: 0
#### quoting single value 4
## tags: posix, quoting
printf '<%s>\n' 'a	b'
## stdout: |
<a	b>
## status: 0
#### quoting double value 4
## tags: posix, quoting
x='a	b'; printf '<%s>\n' "$x"
## stdout: |
<a	b>
## status: 0
#### quoting concatenate value 4
## tags: posix, quoting
x='a	b'; printf '<%s>\n' pre"$x"post
## stdout: |
<prea	bpost>
## status: 0
#### quoting literal-dollar value 4
## tags: posix, quoting
x='a	b'; printf '%s\n' '$x'
## stdout: |
$x
## status: 0
#### quoting escaped-space value 4
## tags: posix, quoting
printf '%s\n' a\ b 'a	b'
## stdout: |
a b
a	b
## status: 0
#### quoting comment-boundary value 4
## tags: posix, quoting
printf '%s\n' a#b 'a	b' # ignored | ;
## stdout: |
a#b
a	b
## status: 0
#### quoting quoted-operators value 4
## tags: posix, quoting
printf '%s\n' '&& || ;; < >' 'a	b'
## stdout: |
&& || ;; < >
a	b
## status: 0
#### quoting empty-adjacent value 4
## tags: posix, quoting
printf '<%s>\n' '''a	b'""
## stdout: |
<a	b>
## status: 0
#### quoting continuation value 4
## tags: posix, quoting
printf '%s\n' ab\
cd 'a	b'
## stdout: |
abcd
a	b
## status: 0
#### quoting escaped-backslash value 4
## tags: posix, quoting
printf '%s\n' \\ 'a	b'
## stdout: |
\
a	b
## status: 0
