use super::*;

fn kinds(input: &[u8]) -> Vec<TokenKind> {
    lex(input)
        .unwrap()
        .into_iter()
        .map(|token| token.kind)
        .collect()
}

fn word(parts: Vec<WordPart>) -> TokenKind {
    TokenKind::Word(Word(parts))
}
fn literal(bytes: &[u8]) -> WordPart {
    WordPart::Literal(bytes.to_vec())
}

#[test]
fn every_operator_and_newline() {
    let source = b"&& || ;; ; & | ( ) < > << >> <& >& <> <<- >| <<<\n";
    let expected = [
        Operator::AndIf,
        Operator::OrIf,
        Operator::DoubleSemicolon,
        Operator::Semicolon,
        Operator::Ampersand,
        Operator::Pipe,
        Operator::LeftParen,
        Operator::RightParen,
        Operator::Less,
        Operator::Greater,
        Operator::HereDoc,
        Operator::Append,
        Operator::DupInput,
        Operator::DupOutput,
        Operator::ReadWrite,
        Operator::HereDocStrip,
        Operator::Clobber,
        Operator::HereString,
    ];
    let mut tokens: Vec<_> = expected.into_iter().map(TokenKind::Operator).collect();
    tokens.push(TokenKind::Newline);
    assert_eq!(kinds(source), tokens);
    assert_eq!(
        kinds(b"&&&|||;;;<<<"),
        vec![
            TokenKind::Operator(Operator::AndIf),
            TokenKind::Operator(Operator::Ampersand),
            TokenKind::Operator(Operator::OrIf),
            TokenKind::Operator(Operator::Pipe),
            TokenKind::Operator(Operator::DoubleSemicolon),
            TokenKind::Operator(Operator::Semicolon),
            TokenKind::Operator(Operator::HereString)
        ]
    );
}

#[test]
fn io_number_requires_unquoted_digits_and_adjacent_redirection() {
    assert_eq!(
        kinds(b"2>file 2 >file"),
        vec![
            TokenKind::IoNumber(b"2".to_vec()),
            TokenKind::Operator(Operator::Greater),
            word(vec![literal(b"file")]),
            word(vec![literal(b"2")]),
            TokenKind::Operator(Operator::Greater),
            word(vec![literal(b"file")])
        ]
    );
    for source in [
        b"'2'>".as_slice(),
        b"\\2>",
        b"2a>",
        b"2\t>",
        b"2\n>",
        b"2|",
        b"2\"\">",
        b"2\\ >",
    ] {
        assert!(
            matches!(kinds(source).first(), Some(TokenKind::Word(_))),
            "{source:?}"
        );
    }
    assert!(matches!(
        kinds(b"123456789012345678901234567890<").first(),
        Some(TokenKind::IoNumber(_))
    ));
    assert!(matches!(
        kinds(b"2\\\n>").first(),
        Some(TokenKind::IoNumber(_))
    ));
}

#[test]
fn positions_are_physical_one_based_byte_coordinates() {
    let tokens = lex(b"\t\xff\n 2>f\\\no\n").unwrap();
    assert_eq!(
        tokens[0].span,
        Span {
            start: Position {
                offset: 1,
                line: 1,
                column: 2
            },
            end: Position {
                offset: 2,
                line: 1,
                column: 3
            }
        }
    );
    assert_eq!(
        tokens[1].span.start,
        Position {
            offset: 2,
            line: 1,
            column: 3
        }
    );
    assert_eq!(
        tokens[2].span.start,
        Position {
            offset: 4,
            line: 2,
            column: 2
        }
    );
    assert_eq!(
        tokens[4].span.end,
        Position {
            offset: 10,
            line: 3,
            column: 2
        }
    );
}

#[test]
fn comments_only_at_word_start() {
    assert_eq!(
        kinds(b"a#b # ignored\n# also ignored\n"),
        vec![
            word(vec![literal(b"a#b")]),
            TokenKind::Newline,
            TokenKind::Newline
        ]
    );
    assert!(kinds(b"# comment").is_empty());
    assert!(kinds(b"").is_empty());
    assert!(kinds(b" \t\\\n").is_empty());
    assert_eq!(
        kinds(b"# comment\\\nnext"),
        vec![TokenKind::Newline, word(vec![literal(b"next")])]
    );
    assert_eq!(
        kinds(b"''#literal"),
        vec![word(vec![
            WordPart::SingleQuoted(vec![]),
            literal(b"#literal")
        ])]
    );
}

#[test]
fn single_quotes_preserve_every_byte_and_empty_quotes() {
    assert_eq!(
        kinds(b"'a$`\\\n\xff' ''"),
        vec![
            word(vec![WordPart::SingleQuoted(b"a$`\\\n\xff".to_vec())]),
            word(vec![WordPart::SingleQuoted(vec![])])
        ]
    );
}

#[test]
fn backslashes_preserve_protection_and_join_lines() {
    assert_eq!(
        kinds(b"a\\ b\\*\\#\\$\\\nc"),
        vec![word(vec![
            literal(b"a"),
            WordPart::Escaped(b' '),
            literal(b"b"),
            WordPart::Escaped(b'*'),
            WordPart::Escaped(b'#'),
            WordPart::Escaped(b'$'),
            literal(b"c")
        ])]
    );
    assert_eq!(
        kinds(b"&\\\n& $na\\\nme"),
        vec![
            TokenKind::Operator(Operator::AndIf),
            word(vec![WordPart::Param(ParamExpr::Name(b"name".to_vec()))])
        ]
    );
}

#[test]
fn double_quotes_keep_parts_and_only_escape_special_bytes() {
    assert_eq!(
        kinds(
            br#""a\q\$\`\"\\*?'#\
Z" """#
        ),
        vec![
            word(vec![WordPart::DoubleQuoted(vec![
                literal(b"a\\q"),
                WordPart::Escaped(b'$'),
                WordPart::Escaped(b'`'),
                WordPart::Escaped(b'"'),
                WordPart::Escaped(b'\\'),
                literal(b"*?'#Z")
            ])]),
            word(vec![WordPart::DoubleQuoted(vec![])])
        ]
    );
}

#[test]
fn parameter_forms_are_not_expanded() {
    assert_eq!(
        kinds(b"$name ${x:-${y}} $1 $? $$ $@ $* $# $- $! $0 $"),
        vec![
            word(vec![WordPart::Param(ParamExpr::Name(b"name".to_vec()))]),
            word(vec![WordPart::Param(ParamExpr::Braced(
                b"x:-${y}".to_vec()
            ))]),
            word(vec![WordPart::Param(ParamExpr::Special(b'1'))]),
            word(vec![WordPart::Param(ParamExpr::Special(b'?'))]),
            word(vec![WordPart::Param(ParamExpr::Special(b'$'))]),
            word(vec![WordPart::Param(ParamExpr::Special(b'@'))]),
            word(vec![WordPart::Param(ParamExpr::Special(b'*'))]),
            word(vec![WordPart::Param(ParamExpr::Special(b'#'))]),
            word(vec![WordPart::Param(ParamExpr::Special(b'-'))]),
            word(vec![WordPart::Param(ParamExpr::Special(b'!'))]),
            word(vec![WordPart::Param(ParamExpr::Special(b'0'))]),
            word(vec![literal(b"$")])
        ]
    );
    assert_eq!(
        kinds(br#"${x:-'}'}"#),
        vec![word(vec![WordPart::Param(ParamExpr::Braced(
            br#"x:-'}'"#.to_vec()
        ))])]
    );
}

#[test]
fn command_substitution_has_an_independent_quote_context() {
    assert_eq!(
        kinds(br#""a$(echo ")")""#),
        vec![word(vec![WordPart::DoubleQuoted(vec![
            literal(b"a"),
            WordPart::CommandSub(CommandSub {
                style: CommandSubStyle::DollarParen,
                source: br#"echo ")""#.to_vec()
            })
        ])])]
    );
    assert_eq!(
        kinds(b"$( (echo $(echo x)); # ) ignored\n echo y )"),
        vec![word(vec![WordPart::CommandSub(CommandSub {
            style: CommandSubStyle::DollarParen,
            source: b" (echo $(echo x)); # ) ignored\n echo y ".to_vec()
        })])]
    );
}

#[test]
fn backticks_keep_raw_source_and_escaped_delimiters() {
    assert_eq!(
        kinds(br"`echo \`nested\` \\$x`"),
        vec![word(vec![WordPart::CommandSub(CommandSub {
            style: CommandSubStyle::Backtick,
            source: br"echo \`nested\` \\$x".to_vec()
        })])]
    );
    assert_eq!(
        kinds(b"``"),
        vec![word(vec![WordPart::CommandSub(CommandSub {
            style: CommandSubStyle::Backtick,
            source: vec![]
        })])]
    );
}

#[test]
fn arithmetic_nesting_is_preserved_without_evaluation() {
    assert_eq!(
        kinds(b"$((1 + (2 * (3)) + $((4 + 5))))"),
        vec![word(vec![WordPart::Arith(
            b"1 + (2 * (3)) + $((4 + 5))".to_vec()
        )])]
    );
    assert_eq!(
        kinds(b"$((1)\\\n)"),
        vec![word(vec![WordPart::Arith(b"1".to_vec())])]
    );
}

#[test]
fn tilde_and_glob_markers_only_when_unquoted() {
    assert_eq!(
        kinds(b"~/x ~user/x x~ *?[ab] '~' \"*?[]\" \\*"),
        vec![
            word(vec![WordPart::Tilde(None), literal(b"/x")]),
            word(vec![
                WordPart::Tilde(Some(b"user".to_vec())),
                literal(b"/x")
            ]),
            word(vec![literal(b"x~")]),
            word(vec![
                WordPart::Glob(GlobChar::Star),
                WordPart::Glob(GlobChar::Question),
                WordPart::Glob(GlobChar::OpenBracket),
                literal(b"ab]")
            ]),
            word(vec![WordPart::SingleQuoted(b"~".to_vec())]),
            word(vec![WordPart::DoubleQuoted(vec![literal(b"*?[]")])]),
            word(vec![WordPart::Escaped(b'*')])
        ]
    );
}

#[test]
fn reserved_words_remain_words() {
    for name in b"if then elif else fi do done case esac while until for in { } ! function"
        .split(|c| *c == b' ')
    {
        assert_eq!(kinds(name), vec![word(vec![literal(name)])]);
    }
}

#[test]
fn all_unclosed_constructs_report_incomplete() {
    for input in [
        b"'".as_slice(),
        b"\"",
        b"$(",
        b"${",
        b"`",
        b"$((",
        b"\\",
        br#""a$(echo ")")"#.as_slice(),
        b"${x:-$(echo}",
    ] {
        assert!(
            matches!(lex(input), Err(LexError::Incomplete(_))),
            "{input:?}"
        );
    }
}

#[test]
fn arbitrary_bytes_and_nul_are_not_decoded() {
    assert_eq!(
        kinds(b"\xff\0\xfe"),
        vec![word(vec![literal(b"\xff\0\xfe")])]
    );
    for byte in 0..=u8::MAX {
        let _ = lex(&[byte]);
    }
}

#[test]
fn deep_nesting_returns_an_error_without_stack_overflow() {
    let mut source = b"$(".repeat(10_000);
    source.extend(b")".repeat(10_000));
    assert!(matches!(lex(&source), Err(LexError::NestingLimit(_))));
    let flat = b"()".repeat(50_000);
    assert_eq!(lex(&flat).unwrap().len(), 100_000);
}

#[test]
fn deterministic_arbitrary_byte_smoke_test() {
    let mut state = 0x87654321u64;
    for len in 0..512 {
        let input: Vec<_> = (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as u8
            })
            .collect();
        if let Ok(tokens) = lex(&input) {
            let mut previous = 0;
            for token in tokens {
                assert!(token.span.start.offset >= previous);
                assert!(token.span.end.offset > token.span.start.offset);
                assert!(token.span.end.offset <= input.len());
                previous = token.span.end.offset;
            }
        }
    }
}

#[test]
fn parameter_quotes_follow_the_enclosing_context() {
    // POSIX 2.2.3: a single quote is ordinary inside a double-quoted
    // non-pattern parameter expansion; substring patterns use their own context.
    assert_eq!(
        kinds(br#""${x:-'}'}""#),
        vec![word(vec![WordPart::DoubleQuoted(vec![
            WordPart::Param(ParamExpr::Braced(b"x:-'".to_vec())),
            literal(b"'}")
        ])])]
    );
    assert_eq!(
        kinds(br#""${x%'}'}""#),
        vec![word(vec![WordPart::DoubleQuoted(vec![WordPart::Param(
            ParamExpr::Braced(b"x%'}'".to_vec())
        )])])]
    );
    assert_eq!(
        kinds(br#""${x:-\}}""#),
        vec![word(vec![WordPart::DoubleQuoted(vec![WordPart::Param(
            ParamExpr::Braced(br"x:-\}".to_vec())
        )])])]
    );
    assert_eq!(
        kinds(br#"${x:-"}"}"#),
        vec![word(vec![WordPart::Param(ParamExpr::Braced(
            br#"x:-"}""#.to_vec()
        ))])]
    );
}

#[test]
fn nested_forms_inside_double_quotes_remain_structured() {
    assert_eq!(
        kinds(b"\"$((1 + $(echo 2)))`echo x`\""),
        vec![word(vec![WordPart::DoubleQuoted(vec![
            WordPart::Arith(b"1 + $(echo 2)".to_vec()),
            WordPart::CommandSub(CommandSub {
                style: CommandSubStyle::Backtick,
                source: b"echo x".to_vec()
            })
        ])])]
    );
    let tokens = lex(b"$(\\\necho x)").unwrap();
    assert_eq!(
        tokens[0].kind,
        word(vec![WordPart::CommandSub(CommandSub {
            style: CommandSubStyle::DollarParen,
            source: b"\\\necho x".to_vec()
        })])
    );
}

#[test]
fn incomplete_errors_identify_the_construct_and_opening_position() {
    let error = lex(b"\n  ${x").unwrap_err();
    assert_eq!(
        error,
        LexError::Incomplete(IncompleteReason {
            construct: "parameter expansion",
            opened_at: Position {
                offset: 3,
                line: 2,
                column: 3
            }
        })
    );
    assert!(error.to_string().starts_with("rsh: lexer:"));
}

#[test]
fn shell_alphabet_random_smoke_test() {
    let alphabet = b"abc012 $'\"`\\(){}#;|&<>*?[]~\n\t\xff\0";
    let mut state = 0x12345678u64;
    for trial in 0..10_000 {
        let input: Vec<_> = (0..trial % 256)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                alphabet[(state as usize) % alphabet.len()]
            })
            .collect();
        let _ = lex(&input);
    }
}

#[test]
fn adjacent_quoted_and_unquoted_parts_form_one_word() {
    assert_eq!(
        kinds(b"pre'$x'\"$name\"\\ tail"),
        vec![word(vec![
            literal(b"pre"),
            WordPart::SingleQuoted(b"$x".to_vec()),
            WordPart::DoubleQuoted(vec![WordPart::Param(ParamExpr::Name(b"name".to_vec()))]),
            WordPart::Escaped(b' '),
            literal(b"tail")
        ])]
    );
}

#[test]
fn heredoc_queue_quotes_tabs_and_body_operators() {
    let tokens = lex(b"cat <<A <<-'B'\n$x ) ' \"\nA\n\t$y\n\tB\necho after\n").unwrap();
    let docs: Vec<_> = tokens
        .iter()
        .filter_map(|t| {
            if let TokenKind::HereDocDelimiter { document, .. } = &t.kind {
                Some(document)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(docs.len(), 2);
    assert_eq!(docs[0].raw, b"$x ) ' \"\n");
    assert!(!docs[0].quoted);
    assert!(matches!(docs[0].parts.first(), Some(WordPart::Param(_))));
    assert_eq!(docs[1].raw, b"$y\n");
    assert!(docs[1].quoted && docs[1].strip_tabs);
    assert_eq!(docs[1].parts, vec![WordPart::Literal(b"$y\n".to_vec())]);
    assert_eq!(tokens.last().unwrap().span.end.line, 7);
}

#[test]
fn heredocs_in_command_substitutions_have_separate_queues() {
    let source = b"echo \"$(cat <<A\n) ' \" ; |\nA\n)\" <<B\nouter\nB\n";
    let tokens = lex(source).unwrap();
    assert!(
        matches!(&tokens[1].kind,TokenKind::Word(Word(parts)) if matches!(parts.first(),Some(WordPart::DoubleQuoted(_))))
    );
    assert_eq!(
        tokens
            .iter()
            .filter(|t| matches!(t.kind, TokenKind::HereDocDelimiter { .. }))
            .count(),
        1
    );
    for input in [
        b"cat <<E".as_slice(),
        b"cat <<E\ntext\n",
        b"echo $(cat <<E\ntext\nE\n",
    ] {
        assert!(matches!(lex(input), Err(LexError::Incomplete(_))));
    }
}

#[test]
fn heredoc_line_joining_respects_backslash_parity() {
    let input = b"cat <<E\nleft\\\nright\nback\\\\\nnext\nE\n";
    let tokens = lex(input).unwrap();
    let doc = tokens
        .iter()
        .find_map(|t| {
            if let TokenKind::HereDocDelimiter { document, .. } = &t.kind {
                Some(document)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(doc.raw, b"leftright\nback\\\\\nnext\n");
}

#[test]
fn here_string_is_one_operator_without_a_heredoc_body() {
    assert_eq!(
        kinds(b"3<<<x\n"),
        vec![
            TokenKind::IoNumber(b"3".to_vec()),
            TokenKind::Operator(Operator::HereString),
            word(vec![literal(b"x")]),
            TokenKind::Newline
        ]
    );
    assert!(lex(b"cat <<< 'word'\n").is_ok());
}
