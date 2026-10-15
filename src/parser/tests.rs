use super::*;

#[test]
fn lists_and_pipelines_keep_precedence_and_background() {
    let tree = parse(b"a && b || c ; d &").unwrap();
    assert_eq!(tree.0.len(), 2);
    assert_eq!(
        tree.0[0]
            .and_or
            .rest
            .iter()
            .map(|(op, _)| *op)
            .collect::<Vec<_>>(),
        [AndOrOp::And, AndOrOp::Or]
    );
    assert_eq!(tree.0[1].separator, Separator::Background);
    let tree = parse(b"! a | b").unwrap();
    assert!(tree.0[0].and_or.first.negated);
    assert_eq!(tree.0[0].and_or.first.commands.len(), 2);
}

#[test]
fn assignments_only_before_first_command_word_and_redirections_keep_order() {
    let tree = parse(b"A=one >out B=\"two\" cmd C=three 2>&1").unwrap();
    let Command::Simple(simple) = &tree.0[0].and_or.first.commands[0] else {
        panic!()
    };
    assert!(matches!(&simple.parts[0], SimplePart::Assignment(a) if a.name == b"A"));
    assert!(matches!(&simple.parts[1], SimplePart::Redirection(r) if r.op == RedirOp::Output));
    assert!(matches!(&simple.parts[2], SimplePart::Assignment(a) if a.name == b"B"));
    assert!(matches!(&simple.parts[3], SimplePart::Word(_)));
    assert!(matches!(&simple.parts[4], SimplePart::Word(_)));
    assert!(
        matches!(&simple.parts[5], SimplePart::Redirection(r) if r.fd.as_deref() == Some(b"2") && r.op == RedirOp::DupOutput)
    );
    for source in [b"'A'=x".as_slice(), b"A\\=x", b"1A=x", b"=x"] {
        let tree = parse(source).unwrap();
        let Command::Simple(simple) = &tree.0[0].and_or.first.commands[0] else {
            panic!()
        };
        assert!(matches!(simple.parts[0], SimplePart::Word(_)));
    }
}

#[test]
fn reserved_words_are_contextual() {
    for source in [
        b"echo if then do done fi in { } !".as_slice(),
        b"'if' arg",
        b"\\if",
        b"A=1 if then",
        b">out if",
        b"for if in if then; do echo if; done",
        b"case x in (if|then|in) echo fi ;; esac",
    ] {
        assert!(parse(source).is_ok(), "{}", String::from_utf8_lossy(source));
    }
    for source in [
        b"then".as_slice(),
        b"fi",
        b"do",
        b"a | ! b",
        b"! ! a",
        b"{ :; } word",
    ] {
        assert!(
            matches!(parse(source), Err(ParseError::Syntax { .. })),
            "{source:?}"
        );
    }
}

#[test]
fn ten_incomplete_syntax_pairs() {
    let pairs: &[(&[u8], &[u8])] = &[
        (b"if", b"if ; then :; fi"),
        (b"if a;", b"if a; else :; fi"),
        (b"if a; then", b"if a; then ; fi"),
        (b"if a; then b; elif", b"if a; then b; elif ; fi"),
        (b"while a;", b"while a; then b; done"),
        (b"until a; do", b"until a; do ; done"),
        (b"case x in", b"case x in |) ;; esac"),
        (b"case x in a", b"case x in a ; esac"),
        (b"(", b"()"),
        (b"{ a;", b"{ ; }"),
        (b"a &&", b"a && ;"),
        (b"a |", b"a | ;"),
        (b"echo >", b"echo > ;"),
        (b"for i; do", b"for 1; do :; done"),
        (b"echo '", b"; echo"),
        (b"echo \"", b"a || || b"),
        (b"echo $(", b"a ;; b"),
        (b"echo ${", b"a & & b"),
        (b"echo `", b"if a; then b; else ; fi"),
    ];
    for (incomplete, syntax) in pairs {
        assert!(
            matches!(parse(incomplete), Err(ParseError::Incomplete(_))),
            "{incomplete:?}: {:?}",
            parse(incomplete)
        );
        assert!(
            matches!(parse(syntax), Err(ParseError::Syntax { .. })),
            "{syntax:?}: {:?}",
            parse(syntax)
        );
    }
}

#[test]
fn syntax_error_location_and_message() {
    let error = parse(b"echo x\n  ;").unwrap_err();
    assert_eq!(
        error,
        ParseError::Syntax {
            line: 2,
            col: 3,
            token: ";".into()
        }
    );
    assert_eq!(
        error.to_string(),
        "rsh: line 2: syntax error near unexpected token ';'"
    );
}

#[test]
fn boundaries_and_newlines() {
    for source in [
        "a &&\nb ||\nc",
        "! a |\nb",
        "if a; then\nb; elif c; then\nd; else\ne; fi",
        "while a; do\nb; done",
        "until a; do\nb; done",
        "for i; do :; done",
        "for i\ndo :; done",
        "for i in; do :; done",
        "for i\nin a b\ndo :; done",
        "case x in (a|b) ;; esac",
        "case x in esac",
        "case x in a)\n;; b) esac",
        "if (false) then (echo x) else (echo y) fi",
        "{ if a; then (while b; do c; done); fi; } 2>out",
    ] {
        let tree = parse(source.as_bytes()).unwrap_or_else(|e| panic!("{source}: {e}"));
        assert_eq!(
            parse(tree.to_string().as_bytes()).unwrap(),
            tree,
            "{source}"
        );
    }
}

#[test]
fn all_redirection_forms_and_empty_commands() {
    for source in [
        "<in",
        ">out",
        "2>>out",
        "0<&3",
        "1>&-",
        "<>file",
        ">|file",
        "X= Y='a'",
        "",
        "# only comment",
    ] {
        let tree = parse(source.as_bytes()).unwrap();
        assert_eq!(parse(&tree.to_source_bytes()).unwrap(), tree);
    }
    for source in ["cat <<EOF", "cat <<-EOF"] {
        assert!(matches!(
            parse(source.as_bytes()),
            Err(ParseError::Incomplete(_))
        ));
    }
    assert!(parse(b"f() { :; }").is_ok());
}

#[test]
fn deep_and_wide_inputs_are_bounded() {
    let mut source = b"(".repeat(10_000);
    source.extend_from_slice(b":");
    source.extend(b")".repeat(10_000));
    assert!(matches!(parse(&source), Err(ParseError::NestingLimit)));
    assert_eq!(parse(&b":;".repeat(10_000)).unwrap().0.len(), 10_000);
}

#[test]
fn non_utf8_words_roundtrip_through_byte_serializer() {
    let tree = parse(b"X=\xff echo '\xfe' >\x80").unwrap();
    assert_eq!(parse(&tree.to_source_bytes()).unwrap(), tree);
}

#[test]
fn arbitrary_byte_smoke_test() {
    let alphabet = b"abc012 $'\"`\\(){}#;|&<>*?[]~\n\t\xff\0";
    let mut state = 123456789u64;
    for trial in 0..10000 {
        let input: Vec<_> = (0..trial % 256)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                alphabet[(state as usize) % alphabet.len()]
            })
            .collect();
        if let Ok(tree) = parse(&input) {
            assert_eq!(parse(&tree.to_source_bytes()).unwrap(), tree, "{input:?}");
        }
    }
}

/// Generate well-formed AST data directly, rather than parsing a fixed source set.
fn generated(seed: &mut u64, depth: usize) -> Script {
    fn next(seed: &mut u64, bound: u64) -> u64 {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (*seed >> 32) % bound
    }
    fn w(text: &[u8]) -> Word {
        Word(vec![WordPart::Literal(text.to_vec())])
    }
    fn simple(seed: &mut u64) -> Command {
        let n = next(seed, 4);
        let mut parts = Vec::new();
        if n == 0 {
            parts.push(SimplePart::Assignment(Assignment {
                name: b"X".to_vec(),
                value: w(b"value"),
            }));
        }
        parts.push(SimplePart::Word(w(b"cmd")));
        parts.push(SimplePart::Word(Word(vec![WordPart::DoubleQuoted(vec![
            WordPart::Param(ParamExpr::Name(b"x".to_vec())),
        ])])));
        if n == 1 {
            parts.push(SimplePart::Redirection(Redirection {
                fd: Some(b"2".to_vec()),
                op: RedirOp::Output,
                target: w(b"file"),
            }));
        }
        Command::Simple(SimpleCommand { parts, line: 1 })
    }
    fn pipe(seed: &mut u64, depth: usize) -> Pipeline {
        let count = 1 + next(seed, 2);
        let mut commands = Vec::new();
        for _ in 0..count {
            if depth == 0 || next(seed, 3) == 0 {
                commands.push(simple(seed));
                continue;
            }
            let compound = match next(seed, 7) {
                0 => CompoundCommand::Brace(generated(seed, depth - 1)),
                1 => CompoundCommand::Subshell(generated(seed, depth - 1)),
                2 => CompoundCommand::While {
                    condition: generated(seed, depth - 1),
                    body: generated(seed, depth - 1),
                },
                3 => CompoundCommand::Until {
                    condition: generated(seed, depth - 1),
                    body: generated(seed, depth - 1),
                },
                4 => CompoundCommand::For {
                    name: b"i".to_vec(),
                    words: if next(seed, 2) == 0 {
                        None
                    } else {
                        Some(vec![w(b"one"), w(b"two")])
                    },
                    body: generated(seed, depth - 1),
                },
                5 => CompoundCommand::If {
                    branches: vec![IfBranch {
                        condition: generated(seed, depth - 1),
                        body: generated(seed, depth - 1),
                    }],
                    else_body: Some(generated(seed, depth - 1)),
                },
                _ => CompoundCommand::Case {
                    word: w(b"x"),
                    items: vec![
                        CaseItem {
                            patterns: vec![w(b"a"), w(b"b")],
                            body: generated(seed, depth - 1),
                        },
                        CaseItem {
                            patterns: vec![w(b"c")],
                            body: Script::default(),
                        },
                    ],
                },
            };
            commands.push(if next(seed, 4) == 0 {
                Command::FunctionDef {
                    name: b"generated_fn".to_vec(),
                    definition: Function {
                        body: Box::new(compound),
                        redirs: Vec::new(),
                    },
                }
            } else {
                Command::Compound(Box::new(compound), Vec::new())
            });
        }
        Pipeline {
            negated: next(seed, 2) == 0,
            commands,
        }
    }
    let count = 1 + next(seed, 2);
    Script(
        (0..count)
            .map(|_| {
                let first = pipe(seed, depth);
                let mut rest = Vec::new();
                for _ in 0..next(seed, 2) {
                    rest.push((
                        if next(seed, 2) == 0 {
                            AndOrOp::And
                        } else {
                            AndOrOp::Or
                        },
                        pipe(seed, depth),
                    ));
                }
                ListItem {
                    and_or: AndOr { first, rest },
                    separator: if next(seed, 3) == 0 {
                        Separator::Background
                    } else {
                        Separator::Sequential
                    },
                }
            })
            .collect(),
    )
}

#[test]
fn generated_ast_display_parse_property() {
    let mut seed = 0x12345678;
    for _ in 0..200 {
        let tree = generated(&mut seed, 2);
        let source = tree.to_string();
        assert_eq!(
            parse(source.as_bytes()).unwrap_or_else(|e| panic!("{source}: {e}")),
            tree
        );
    }
}

#[test]
fn for_omitted_in_uses_exactly_one_separator_or_none() {
    assert!(parse(b"for i do :; done").is_ok());
    assert!(matches!(
        parse(b"for i\n; do :; done"),
        Err(ParseError::Syntax { .. })
    ));
    assert!(matches!(
        parse(b"for i\n\n; do :; done"),
        Err(ParseError::Syntax { .. })
    ));
}

#[test]
fn heredoc_ast_roundtrips_nested_groups_pipelines_and_substitutions() {
    for source in [
        "cat <<A <<-'B'\nfirst\nA\n\tsecond\n\tB\n",
        "{ cat <<E; echo done; } >out\n$x\nE\n",
        "cat <<A | { cat <<B; cat; }\none\nA\ntwo\nB\n",
        "echo \"$(cat <<EOF\n) not syntax\nEOF\n)\"\n",
        "cat <<''\n\n",
        "cat <<E\\OF\n$x\nEOF\n",
        "(cat) <<E\ncontent\nE\n",
        "if cat <<E\nx\nE\nthen :; fi",
    ] {
        let ast = parse(source.as_bytes()).unwrap_or_else(|e| panic!("{source}: {e}"));
        let displayed = ast.to_string();
        assert_eq!(
            parse(displayed.as_bytes()).unwrap_or_else(|e| panic!("{displayed}: {e}")),
            ast
        );
    }
    for source in [
        "cat <<E",
        "cat <<E\n",
        "cat <<E\nwrong\n",
        "cat <<A <<B\na\nA\nb\n",
    ] {
        assert!(matches!(
            parse(source.as_bytes()),
            Err(ParseError::Incomplete(_))
        ));
    }
}

#[test]
fn functions_require_names_compound_bodies_and_complete_headers() {
    for source in [
        "f() { :; }",
        "f ()\nif true; then :; fi >out",
        "f() ( : )",
        "f(){ g(){ :; }; g; }",
        "f(){ cat <<END\nbody\nEND\n} >out",
    ] {
        let ast = parse(source.as_bytes()).unwrap();
        assert_eq!(parse(&ast.to_source_bytes()).unwrap(), ast);
    }
    for source in ["f", "f() {", "f()\n", "f() if true; then :;"] {
        if source != "f" {
            assert!(
                matches!(parse(source.as_bytes()), Err(ParseError::Incomplete(_))),
                "{source}"
            );
        }
    }
    for source in [
        "f() word",
        "'f'() { :; }",
        "1x() { :; }",
        "exit() { :; }",
        "return() { :; }",
        "f(x) { :; }",
        "{ (true) }",
    ] {
        assert!(
            matches!(parse(source.as_bytes()), Err(ParseError::Syntax { .. })),
            "{source}: {:?}",
            parse(source.as_bytes())
        );
    }
}
