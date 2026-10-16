#!/usr/bin/env python3
"""Original POSIX-derived matrix, not imported from Oils or generated from rsh.

Expectations are constructed from the stated grammar/semantics. Run each area
through verify-references before using rsh results. Keep this authoring source so
matrix coverage and literal expectations are reviewable together.
"""
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parent
AREAS = {}

def q(s): return "'" + s.replace("'", "'\\''") + "'"
def add(area, name, script, out='', status=0, stderr=None, alt=None, skip=None, note=''):
    AREAS.setdefault(area, []).append((name, script, out, status, stderr, alt or {}, skip or {}, note))
def fields(values): return ''.join('<' + x + '>\n' for x in values)

# POSIX 2.2 and 2.3: escaping, concatenation, quoting, comments, operators.
values = ['', 'a b', '*?[ab]', '$x;#&|', 'a\tb']
for i, value in enumerate(values):
    for kind, script, expected in [
        ('single', f"printf '<%s>\\n' {q(value)}", fields([value])),
        ('double', f"x={q(value)}; printf '<%s>\\n' \"$x\"", fields([value])),
        ('concatenate', f"x={q(value)}; printf '<%s>\\n' pre\"$x\"post", fields(['pre'+value+'post'])),
        ('literal-dollar', f"x={q(value)}; printf '%s\\n' '\u0024x'", '$x\n'),
        ('escaped-space', f"printf '%s\\n' a\\ b {q(value)}", 'a b\n'+value+'\n'),
        ('comment-boundary', f"printf '%s\\n' a#b {q(value)} # ignored | ;\n", 'a#b\n'+value+'\n'),
        ('quoted-operators', f"printf '%s\\n' '&& || ;; < >' {q(value)}", '&& || ;; < >\n'+value+'\n'),
        ('empty-adjacent', f"printf '<%s>\\n' ''{q(value)}\"\"", fields([value])),
        ('continuation', f"printf '%s\\n' ab\\\ncd {q(value)}", 'abcd\n'+value+'\n'),
        ('escaped-backslash', f"printf '%s\\n' \\\\ {q(value)}", '\\\n'+value+'\n'),
    ]: add('quoting', f'quoting {kind} value {i}', script, expected)

# POSIX 2.3.1, 2.4: names in argument position are not reserved; aliases in script input.
for word in ['if','then','else','fi','while','do','done','case','esac','in']:
    add('reserved-aliases', 'reserved argument '+word, f"printf '<%s>\\n' {word}", fields([word]))
for i in range(5):
    add('reserved-aliases', f'alias invocation {i}', f"alias rsh_m10_aa='printf ALIAS{i}'\nrsh_m10_aa\nprintf '\\n'", f'ALIAS{i}\n')
    add('reserved-aliases', f'alias trailing blank {i}', f"alias rsh_m10_aa='printf \"%s\\n\" ' rsh_m10_bb='VALUE{i}'\nrsh_m10_aa rsh_m10_bb", f'VALUE{i}\n')
    add('reserved-aliases', f'unalias {i}', f"alias rsh_m10_aa='printf {i}'\nunalias rsh_m10_aa\nif command -v rsh_m10_aa >/dev/null; then printf 'found\\n'; else printf 'absent\\n'; fi", 'absent\n')
    add('reserved-aliases', f'quoted command bypasses alias {i}', f"alias printf='false'\n'printf' '%s\\n' {i}", str(i)+'\n')

# POSIX 2.5/2.6.2: set/null distinctions and positional parameters.
for i, value in enumerate(['', 'word', 'a b', '*', 'é']):
    for kind, script, expected in [
        ('set', f"v={q(value)}; printf '<%s>\\n' \"$v\"", fields([value])),
        ('unset-default', f"unset v; printf '<%s>\\n' \"${{v-{value}}}\"", fields([value])),
        ('null-no-colon', f"v=; printf '<%s>\\n' \"${{v-{value}}}\"", fields([''])),
        ('null-colon', f"v=; printf '<%s>\\n' \"${{v:-{value}}}\"", fields([value])),
        ('alternate', f"v=x; printf '<%s>\\n' \"${{v:+{value}}}\"", fields([value])),
        ('no-alternate', f"unset v; printf '<%s>\\n' \"${{v+{value}}}\"", fields([''])),
        ('assign-default', f"unset v; : \"${{v:={value}}}\"; printf '<%s>\\n' \"$v\"", fields([value])),
        ('length', f"v={q(value)}; printf '%s\\n' \"${{#v}}\"", str(len(value.encode()))+'\n'),
        ('positional', f"set -- {q(value)} two; printf '%s:<%s>:<%s>\\n' \"$#\" \"$1\" \"$2\"", f'2:<{value}>:<two>\n'),
        ('quoted-at', f"set -- {q(value)} two; printf '<%s>\\n' \"$@\"", fields([value,'two'])),
        ('quoted-star', f"set -- {q(value)} two; IFS=:; printf '<%s>\\n' \"$*\"", fields([value+':two'])),
        ('status-special', f"(exit {i}); printf '%s\\n' \"$?\"", str(i)+'\n'),
    ]: add('parameters', f'parameter {kind} value {i}', script, expected)

# POSIX 2.6: 60 arithmetic precedence/associativity cases, 60 other expansions.
for i in range(1, 6):
    arithmetic = [
        (f'{i}+2*3',i+6),(f'({i}+2)*3',(i+2)*3),(f'{i}*7/2',i*7//2),
        (f'{i}*7%4',i*7%4),(f'{i}<<2',i<<2),(f'{i}+1<<2',(i+1)<<2),
        (f'{i}|8',i|8),(f'{i}^3',i^3),(f'{i}&3',i&3),(f'{i}>2 ? 7 : 9',7 if i>2 else 9),
        (f'0 && 1/0 || {i}>0',1),(f'(a={i})+(a+=2)',2*i+2)]
    for j,(expr,result) in enumerate(arithmetic):
        add('expansion', f'arithmetic precedence {i}-{j}', f"printf '%s\\n' $(({expr}))", str(result)+'\n', alt={'bash': {'stdout': '', 'status': 1, 'stderr-match': 'division by 0'}} if j == 10 else None, note='POSIX 2.6.4/C short-circuiting: dash result is normative; Apple bash 3.2 evaluates the unreachable division (documented reference bug).' if j == 10 else '')
    for kind, script, expected in [
        ('cmd-trailing-newlines',f"v=$(printf '{i}\\n\\n'); printf '<%s>\\n' \"$v\"",fields([str(i)])),
        ('cmd-inner-newline',f"v=$(printf 'a\\n{i}\\n'); printf '<%s>\\n' \"$v\"",fields(['a\n'+str(i)])),
        ('nested-substitution',f"printf '%s\\n' \"$(printf '%s' \"$(printf {i})\")\"",str(i)+'\n'),
        ('backticks',f"printf '%s\\n' \"`printf {i}`\"",str(i)+'\n'),
        ('substitution-scope',f"v={i}; x=$(v=other; printf x); printf '%s:%s\\n' \"$v\" \"$x\"",f'{i}:x\n'),
        ('substitution-status',f"v=$(exit {i}); printf '%s\\n' $?",str(i)+'\n'),
        ('tilde-home',f"HOME=home{i}; printf '%s\\n' ~",f'home{i}\n'),
        ('assignment-tilde',f"HOME=home{i}; v=~:~; printf '%s\\n' \"$v\"",f'home{i}:home{i}\n'),
        ('short-prefix',f"v=a{i}bcabc; printf '%s\\n' \"${{v#a*}}\"",f'{i}bcabc\n'),
        ('long-prefix',f"v=a{i}bcabc; printf '%s\\n' \"${{v##a*}}\"",'\n'),
        ('short-suffix',f"v=abc{i}abc; printf '%s\\n' \"${{v%c*}}\"",f'abc{i}ab\n'),
        ('long-suffix',f"v=abc{i}abc; printf '%s\\n' \"${{v%%c*}}\"",'ab\n'),
    ]: add('expansion', f'expansion {kind} {i}', script, expected)

# POSIX 2.6.5/2.6.6/2.6.7 and 2.14 patterns.
for i,(value,expect) in enumerate([('',[]),('a',['a']),(' a ',['a']),('a  b',['a','b']),('a\tb\nc',['a','b','c']),('   ',[]),('a:b',['a:b']),('a*',['a*']),('\ta\t',['a']),('a\nb\n',['a','b'])]):
    add('splitting-globbing', f'default IFS {i}', f"v={q(value)}; set -- $v; printf '%s\\n' \"$#\"; for x; do printf '<%s>\\n' \"$x\"; done", str(len(expect))+'\n'+fields(expect))
for i,(value,expect) in enumerate([('',[]),(':',['']),('::',['','']),(':a',['','a']),('a:',['a']),('a::b',['a','','b']),(':a:',['','a']),('a:b:c',['a','b','c']),('a::',['a','']),(' : ',[' ',' '])]):
    add('splitting-globbing', f'nonwhite IFS {i}', f"IFS=:; v={q(value)}; set -- $v; printf '%s\\n' \"$#\"; for x; do printf '<%s>\\n' \"$x\"; done", str(len(expect))+'\n'+fields(expect))
for i,value in enumerate(['','a b','a:b','*','\n']):
    add('splitting-globbing',f'IFS disabled {i}',f"set -f; IFS=; v={q(value)}; set -- $v; printf '%s\\n' \"$#\"; for x; do printf '<%s>\\n' \"$x\"; done", ('0\n' if value=='' else '1\n'+fields([value])))
    add('splitting-globbing',f'quoted empty retained {i}',f"v={q(value)}; set -- \"$v\"; printf '%s\\n' \"$#\"; printf '<%s>\\n' \"$1\"",'1\n'+fields([value]))
files=['a.txt','b.txt','c1','c2','.dot','alpha','beta']
setup='mkdir patterns; cd patterns; '+ '; '.join('printf x >'+q(f) for f in files)+'; '
patterns=[('*.txt',['a.txt','b.txt']),('c?',['c1','c2']),('[ab]*',['a.txt','alpha','b.txt','beta']),('[!a.]*',['b.txt','beta','c1','c2']),('.d*',['.dot']),('missing*',['missing*']),('[[:digit:]]*',['[[:digit:]]*']),('c[12]',['c1','c2']),('a*',['a.txt','alpha']),('b*',['b.txt','beta'])]
for i,(pattern,expect) in enumerate(patterns):
    add('splitting-globbing',f'glob pattern {i}',setup+f"printf '<%s>\\n' {pattern}",fields(expect))
    add('splitting-globbing',f'glob quoted pattern {i}',setup+f"printf '<%s>\\n' {q(pattern)}",fields([pattern]))
    add('splitting-globbing',f'glob variable result {i}',setup+f"v={q(pattern)}; printf '<%s>\\n' $v",fields(expect))
    add('splitting-globbing',f'glob disabled pattern {i}',setup+f"set -f; printf '<%s>\\n' {pattern}",fields([pattern]))

# POSIX 2.7: exact fd ordering and here-document semantics.
for i in range(5):
    for kind,script,out in [
        ('output',f"printf '%s\\n' {i} >f; cat <f",f'{i}\n'),
        ('append',f"printf a >f; printf {i} >>f; cat f; printf '\\n'",f'a{i}\n'),
        ('force',f"printf a >f; set -C; printf {i} >|f; cat f; printf '\\n'",f'{i}\n'),
        ('dup-order-one',f"{{ printf O{i}; printf E{i} >&2; }} >f 2>&1; cat f; printf '\\n'",f'O{i}E{i}\n'),
        ('dup-order-two',f"{{ printf O{i}; printf E{i} >&2; }} 2>&1 >f; cat f; printf '\\n'",f'E{i}O{i}\n'),
        ('persistent-fd',f"exec 3>f; printf {i} >&3; exec 3>&-; cat f; printf '\\n'",f'{i}\n'),
        ('read-write',f"printf {i} >f; cat 0<>f; printf '\\n'",f'{i}\n'),
        ('closed-input',f"printf {i} 0<&-; printf '\\n'",f'{i}\n'),
        ('brace-redir',f"{{ printf {i}; }} >f; cat f; printf '\\n'",f'{i}\n'),
        ('subshell-redir',f"printf {i} >f; (cat) <f; printf '\\n'",f'{i}\n'),
        ('heredoc',f"v={i}\ncat <<EOF\n$v\nEOF",f'{i}\n'),
        ('quoted-heredoc',f"v={i}\ncat <<'EOF'\n$v\nEOF",'$v\n'),
        ('strip-tabs',f"v={i}\ncat <<-EOF\n\t$v\n\tEOF",f'{i}\n'),
        ('multiple-documents',f"cat <<ONE <<TWO\nunused\nONE\n{i}\nTWO",f'{i}\n'),
    ]: add('redirection-heredoc',f'redirection {kind} {i}',script,out)

# POSIX 2.8 and 2.9: statuses are deliberately not normalized.
for i in range(5):
    fallback = "printf 'FALLBACK{}\\n'".format(i)
    for kind,script,status,out in [
        ('exit',f'exit {i}',i,''),('last',f'false; (exit {i})',i,''),
        ('pipeline-last',f'(exit {i}) | true',0,''),('pipeline-fail',f'true | (exit {i})',i,''),
        ('negate',f'! (exit {i})',int(i==0),''),
        ('missing',f'no_such_rsh_m10_command_{i}',127,''),
        ('permission',f'printf x >not-executable-{i}; ./not-executable-{i}',126,''),
        ('script-fallback',f"printf '%s\\n' {q(fallback)} >script; chmod +x script; ./script",0,f'FALLBACK{i}\n'),
    ]: add('status-errors',f'status {kind} {i}',script,out,status)

# POSIX 2.9 compounds, conditions, lists, functions and scope.
for i in range(5):
    for kind,script,out in [
        ('and-or',f"false && printf BAD || printf {i}; printf '\\n'",f'{i}\n'),
        ('left-association',f"true || printf BAD && printf {i}; printf '\\n'",f'{i}\n'),
        ('pipeline-five',f"printf '{i}\\n' | cat | cat | cat | cat",f'{i}\n'),
        ('negated-pipeline',f"if ! false | false; then printf '{i}\\n'; fi",f'{i}\n'),
        ('if-elif',f"if false; then :; elif true; then printf '{i}\\n'; else :; fi",f'{i}\n'),
        ('empty-branch-status',f"if false; then exit {i}; fi; printf '%s\\n' $?",'0\n'),
        ('while',f"n=0; while test $n -lt {i}; do n=$((n+1)); done; printf '%s\\n' $n",f'{i}\n'),
        ('until',f"n=0; until test $n -ge {i}; do n=$((n+1)); done; printf '%s\\n' $n",f'{i}\n'),
        ('for',f"for x in a {i} b; do printf '<%s>\\n' \"$x\"; done",fields(['a',str(i),'b'])),
        ('for-positional',f"set -- {i} 'a b'; for x; do printf '<%s>\\n' \"$x\"; done",fields([str(i),'a b'])),
        ('case',f"case x{i} in (z) printf BAD;; (x[0-9]|other) printf '{i}\\n';; esac",f'{i}\n'),
        ('function-args',f"set -- outer; f(){{ printf '%s:%s\\n' \"$#\" \"$1\"; }}; f {i}; printf '%s\\n' \"$1\"",f'1:{i}\nouter\n'),
        ('function-return',f"f(){{ return {i}; printf BAD; }}; f; printf '%s\\n' $?",f'{i}\n'),
        ('subshell-scope',f"v={i}; (v=other); printf '%s\\n' $v",f'{i}\n'),
        ('brace-scope',f"v=old; {{ v={i}; }}; printf '%s\\n' $v",f'{i}\n'),
        ('break-two',f"for a in x y; do for b in 1 2; do printf '{i}\\n'; break 2; done; done",f'{i}\n'),
    ]: add('commands',f'command {kind} {i}',script,out)

# POSIX 2.10: 15 incomplete/invalid scripts and 15 valid boundary productions.
invalid=[')', '&& :', ': |', 'if true; then', 'while :; do', 'for', 'case x in x)', 'f()', '{ :;', '( :', "printf 'open", 'printf "open', 'printf ${v', 'printf $(printf x', 'printf `printf x']
for i,script in enumerate(invalid): add('syntax',f'syntax invalid {i}',script,'',2)
valid=['{ (:); }','{ (:) }','if (:) then :; fi','case x in esac','case x in (a|b) ;; esac','for i; do :; done','! true | false','true &&\n true','true |\n true','if true; then\n :; fi','while false; do\n :; done','case x in x) :; esac','f()\n{ :; }; f','case x in x)\n;; esac','for i in; do :; done']
for i,script in enumerate(valid): add('syntax',f'syntax valid {i}',script,'',0)

# POSIX utility definitions: 24 families x 5 representative operands.
for i in range(5):
    families=[
        ('colon',f': ignored {i}', '',0),('true',f'true {i}','',0),('false',f'false {i}','',1),
        ('printf-decimal',f"printf '%05d\\n' {i}",f'{i:05d}\n',0),
        ('printf-octal',f"printf '%o\\n' {i+8}",f'{i+8:o}\n',0),
        ('printf-hex',f"printf '%X\\n' {i+250}",f'{i+250:X}\n',0),
        ('printf-repeat',f"printf '<%s>\\n' a {i} b",fields(['a',str(i),'b']),0),
        ('printf-b',f"printf '%b' 'a\\n{i}\\n'",f'a\n{i}\n',0),
        ('echo',f'echo a {i}',f'a {i}\n',0),
        ('test-number',f"test {i} -eq {i}; printf '%s\\n' $?",'0\n',0),
        ('test-string',f"[ x{i} != y{i} ]; printf '%s\\n' $?",'0\n',0),
        ('test-negation',f"test ! {q(str(i))}; printf '%s\\n' $?",'1\n',0),
        ('export',f"export VV={i}; export -p >vars; test -s vars; printf '%s\\n' $VV",f'{i}\n',0),
        ('readonly',f"readonly VV={i}; readonly -p >vars; test -s vars; printf '%s\\n' $VV",f'{i}\n',0),
        ('unset',f"VV={i}; unset -v VV; printf '%s\\n' \"${{VV-unset}}\"",'unset\n',0),
        ('shift',f"set -- a {i} b; shift; printf '%s:<%s>\\n' \"$#\" \"$1\"",f'2:<{i}>\n',0),
        ('eval',f"v={i}; eval 'printf \"%s\\n\" \"$v\"'",f'{i}\n',0),
        ('exec',f'exec /usr/bin/printf "%s\\n" {i}',f'{i}\n',0),
        ('read',f"printf 'a:{i}:b\\n' | {{ IFS=: read -r a b; printf '<%s>:<%s>\\n' \"$a\" \"$b\"; }}",f'<a>:<{i}:b>\n',0),
        ('getopts',f"set -- -a {i}; getopts ab: opt; printf '%s:%s\\n' $opt $OPTIND",'a:2\n',0),
        ('cd',f"mkdir d{i}; cd d{i}; printf '%s\\n' \"${{PWD##*/}}\"",f'd{i}\n',0),
        ('pwd',f"mkdir d{i}; cd d{i}; cd -P .; test \"$(pwd -P)\" = \"$PWD\"; printf '%s\\n' $?",'0\n',0),
        ('command-function',f"printf(){{ false; }}; command printf '%s\\n' {i}",f'{i}\n',0),
        ('umask',f"umask 077; printf {i} >f; test -r f; printf '%s\\n' $?",'0\n',0),
    ]
    for kind,script,out,status in families: add('builtins',f'builtin {kind} {i}',script,out,status)

# POSIX set options plus explicitly tagged pipefail extension.
for i in range(5):
    for kind,script,out,status,stderr in [
        ('errexit',f"set -e; false; printf BAD{i}",'',1,None),
        ('errexit-condition',f"set -e; if false; then :; fi; printf '{i}\\n'",f'{i}\n',0,None),
        ('errexit-function-exempt',f"set -e; f(){{ false; printf '{i}\\n'; }}; if f; then :; fi",f'{i}\n',0,None),
        ('nounset-at',f"set -u; set --; printf '%s:{i}\\n' \"$#\"; for x; do :; done",f'0:{i}\n',0,None),
        ('xtrace',f"PS4='trace:'; set -x; printf '{i}\\n'",f'{i}\n',0,'trace:'),
        ('noglob',f"printf x >f{i}; set -f; printf '%s\\n' f*",'f*\n',0,None),
        ('noexec',f"set -n; printf BAD{i}",'',0,None),
        ('noclobber',f"printf old >f; set -C; (printf {i} >f) 2>/dev/null; cat f; printf '\\n'",'old\n',0,None),
    ]: add('options',f'option {kind} {i}',script,out,status,stderr)

for i in range(1,6):
    add('options', f'pipefail extension {i}', f"set -o pipefail; (exit {i}) | true; printf '%s\\n' $?", str(i)+'\n', skip={'dash':'Apple dash-16 has no pipefail option'}, note='Extension: pipefail is supported by bash and rsh; absent in the recorded dash reference.')

# POSIX noninteractive wait, background environment, EXIT and signal traps.
for i in range(5):
    for kind,script,out in [
        ('wait-status',f"(exit {i}) & p=$!; wait $p; printf '%s\\n' $?",f'{i}\n'),
        ('wait-unknown',f"wait {900000+i} 2>/dev/null; printf '%s\\n' $?",'127\n'),
        ('wait-all',f"(exit {i}) & true & wait; printf '%s\\n' $?",'0\n'),
        ('background-scope',f"v={i}; (v=other) & wait; printf '%s\\n' $v",f'{i}\n'),
        ('exit-trap',f"trap 'printf \"EXIT{i}\\n\"' 0; :",f'EXIT{i}\n'),
        ('usr1-trap',f"trap 'printf \"CAUGHT{i}\\n\"' USR1; kill -s USR1 $$; :",f'CAUGHT{i}\n'),
    ]: add('jobs-signals',f'job {kind} {i}',script,out)


def write():
    for area,cases in AREAS.items():
        directory=ROOT/area; directory.mkdir(exist_ok=True)
        output=['# Original POSIX-derived cases. No Oils source copied.\n', '# Reference: https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html\n']
        for name,script,out,status,stderr,alt,skip,note in cases:
            output += ['#### '+name+'\n', '## tags: '+('extension' if name.startswith('pipefail extension') else 'posix')+', '+area+'\n']
            if note: output.append('# '+note+'\n')
            output += [script.rstrip('\n')+'\n','## stdout: |\n',out]
            if out and not out.endswith('\n'):
                raise ValueError('matrix stdout must end with newline: '+name)
            output.append('## status: '+str(status)+'\n')
            if stderr is not None: output.append('## stderr-match: '+stderr+'\n')
            for shell,value in alt.items(): output.append('## OK '+shell+': '+json.dumps(value,ensure_ascii=False)+'\n')
            for shell,note in skip.items(): output.append('## N-I '+shell+': '+note+'\n')
        (directory/'matrix.test.sh').write_text(''.join(output))
        print(area,len(cases))
if __name__=='__main__': write()
