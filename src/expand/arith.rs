//! Signed 64-bit arithmetic, wrapping on overflow; lazy logical/conditional evaluation.
use super::ExpandError;
use crate::env::ShellEnv;
#[derive(Debug)]
enum Expr {
    Number(i64),
    Var(Vec<u8>),
    Unary(Vec<u8>, Box<Expr>),
    Binary(Vec<u8>, Box<Expr>, Box<Expr>),
    Conditional(Box<Expr>, Box<Expr>, Box<Expr>),
    Assign(Vec<u8>, Vec<u8>, Box<Expr>),
}
struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    depth: usize,
    nodes: usize,
}
impl Parser<'_> {
    fn ws(&mut self) {
        while self.s.get(self.i).is_some_and(u8::is_ascii_whitespace) {
            self.i += 1;
        }
    }
    fn take(&mut self, b: &[u8]) -> bool {
        self.ws();
        if self.s.get(self.i..self.i + b.len()) == Some(b) {
            self.i += b.len();
            true
        } else {
            false
        }
    }
    fn expr(&mut self, min: u8) -> Result<Expr, ExpandError> {
        self.depth += 1;
        self.nodes += 1;
        if self.nodes > 256 {
            return Err(ExpandError::new("arithmetic expression resource limit"));
        }
        if self.depth > 64 {
            return Err(ExpandError::new("arithmetic nesting limit"));
        }
        self.ws();
        let mut lhs = if let Some(op) = [b"+".as_slice(), b"-", b"!", b"~"]
            .iter()
            .find(|op| self.s.get(self.i..self.i + op.len()) == Some(**op))
            .copied()
        {
            self.i += op.len();
            Expr::Unary(op.to_vec(), Box::new(self.expr(14)?))
        } else if self.take(b"(") {
            let e = self.expr(1)?;
            if !self.take(b")") {
                return Err(ExpandError::new("arithmetic: missing )"));
            }
            e
        } else {
            self.ws();
            let start = self.i;
            while self
                .s
                .get(self.i)
                .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
            {
                self.i += 1;
            }
            let word = self.s.get(start..self.i).unwrap_or_default();
            if word.is_empty() {
                return Err(ExpandError::new("arithmetic: expected operand"));
            }
            if word.first().is_some_and(u8::is_ascii_digit) {
                Expr::Number(number(word)?)
            } else {
                Expr::Var(word.to_vec())
            }
        };
        loop {
            self.ws();
            if min <= 3 && self.take(b"?") {
                let yes = self.expr(1)?;
                if !self.take(b":") {
                    return Err(ExpandError::new("arithmetic: missing :"));
                }
                let no = self.expr(3)?;
                lhs = Expr::Conditional(Box::new(lhs), Box::new(yes), Box::new(no));
                continue;
            }
            let ops: &[(&[u8], u8)] = &[
                (b",", 1),
                (b"||", 4),
                (b"&&", 5),
                (b"|", 6),
                (b"^", 7),
                (b"&", 8),
                (b"==", 9),
                (b"!=", 9),
                (b"<=", 10),
                (b">=", 10),
                (b"<<", 11),
                (b">>", 11),
                (b"<", 10),
                (b">", 10),
                (b"+", 12),
                (b"-", 12),
                (b"*", 13),
                (b"/", 13),
                (b"%", 13),
                (b"=", 2),
                (b"+=", 2),
                (b"-=", 2),
                (b"*=", 2),
                (b"/=", 2),
                (b"%=", 2),
                (b"<<=", 2),
                (b">>=", 2),
                (b"&=", 2),
                (b"^=", 2),
                (b"|=", 2),
            ];
            let found = ops
                .iter()
                .filter(|(op, _)| {
                    !op.is_empty() && self.s.get(self.i..self.i + op.len()) == Some(*op)
                })
                .max_by_key(|(op, _)| op.len())
                .copied();
            let Some((op, prec)) = found else {
                break;
            };
            if prec < min {
                break;
            }
            self.i += op.len();
            let rhs = self.expr(if prec == 2 { prec } else { prec + 1 })?;
            lhs = if prec == 2 {
                let Expr::Var(name) = lhs else {
                    return Err(ExpandError::new(
                        "arithmetic: assignment requires a variable",
                    ));
                };
                Expr::Assign(name, op.to_vec(), Box::new(rhs))
            } else {
                Expr::Binary(op.to_vec(), Box::new(lhs), Box::new(rhs))
            };
        }
        self.depth -= 1;
        Ok(lhs)
    }
}
fn number(s: &[u8]) -> Result<i64, ExpandError> {
    let (base, digits) = if s.starts_with(b"0x") || s.starts_with(b"0X") {
        (16, s.get(2..).unwrap_or_default())
    } else if s.len() > 1 && s.first() == Some(&b'0') {
        (8, s.get(1..).unwrap_or_default())
    } else {
        (10, s)
    };
    if digits.is_empty() {
        return Err(ExpandError::new("arithmetic: invalid integer"));
    }
    digits.iter().try_fold(0i64, |v, b| {
        let d = (*b as char)
            .to_digit(base)
            .ok_or_else(|| ExpandError::new("arithmetic: invalid integer"))?;
        Ok(v.wrapping_mul(i64::from(base)).wrapping_add(i64::from(d)))
    })
}
pub fn eval(source: &[u8], env: &mut ShellEnv) -> Result<i64, ExpandError> {
    eval_depth(source, env, 0)
}
fn eval_depth(source: &[u8], env: &mut ShellEnv, depth: usize) -> Result<i64, ExpandError> {
    if depth > 64 {
        return Err(ExpandError::new("arithmetic variable recursion limit"));
    }
    if source.iter().all(u8::is_ascii_whitespace) {
        return Ok(0);
    }
    let mut p = Parser {
        s: source,
        i: 0,
        depth: 0,
        nodes: 0,
    };
    let e = p.expr(1)?;
    p.ws();
    if p.i != source.len() {
        return Err(ExpandError::new("arithmetic: unexpected token"));
    }
    value(&e, env, depth)
}
fn value(e: &Expr, env: &mut ShellEnv, depth: usize) -> Result<i64, ExpandError> {
    if depth > 128 {
        return Err(ExpandError::new("arithmetic evaluation nesting limit"));
    }
    Ok(match e {
        Expr::Number(n) => *n,
        Expr::Var(n) => {
            if env.get(n).is_none() && env.options.nounset {
                return Err(ExpandError::new("arithmetic: parameter not set"));
            }
            let source = env.get(n).unwrap_or(b"0").to_vec();
            eval_depth(&source, env, depth + 1)?
        }
        Expr::Assign(name, op, rhs) => {
            let rhs = value(rhs, env, depth + 1)?;
            let result = if op == b"=" {
                rhs
            } else {
                let source = env.get(name).unwrap_or(b"0").to_vec();
                let old = eval_depth(&source, env, depth + 1)?;
                value(
                    &Expr::Binary(
                        op.get(..op.len().saturating_sub(1))
                            .unwrap_or_default()
                            .to_vec(),
                        Box::new(Expr::Number(old)),
                        Box::new(Expr::Number(rhs)),
                    ),
                    env,
                    depth + 1,
                )?
            };
            env.assign(name.clone(), result.to_string().into_bytes())
                .map_err(ExpandError::readonly)?;
            result
        }
        Expr::Unary(op, e) => {
            let v = value(e, env, depth + 1)?;
            match op.as_slice() {
                b"-" => v.wrapping_neg(),
                b"!" => i64::from(v == 0),
                b"~" => !v,
                _ => v,
            }
        }
        Expr::Conditional(c, a, b) => value(
            if value(c, env, depth + 1)? != 0 { a } else { b },
            env,
            depth,
        )?,
        Expr::Binary(op, a, b) => {
            let a = value(a, env, depth + 1)?;
            if op == b"&&" && a == 0 {
                return Ok(0);
            }
            if op == b"||" && a != 0 {
                return Ok(1);
            }
            let b = value(b, env, depth + 1)?;
            match op.as_slice() {
                b"+" => a.wrapping_add(b),
                b"-" => a.wrapping_sub(b),
                b"*" => a.wrapping_mul(b),
                b"/" | b"%" if b == 0 => {
                    return Err(ExpandError::new("arithmetic: division by zero"));
                }
                b"/" => a.wrapping_div(b),
                b"%" => a.wrapping_rem(b),
                b"<<" => a.wrapping_shl(b as u32),
                b">>" => a.wrapping_shr(b as u32),
                b"<" => i64::from(a < b),
                b"<=" => i64::from(a <= b),
                b">" => i64::from(a > b),
                b">=" => i64::from(a >= b),
                b"==" => i64::from(a == b),
                b"!=" => i64::from(a != b),
                b"&" => a & b,
                b"^" => a ^ b,
                b"|" => a | b,
                b"&&" | b"||" => i64::from(b != 0),
                b"," => b,
                _ => return Err(ExpandError::new("arithmetic: invalid operator")),
            }
        }
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arithmetic() {
        let mut e = ShellEnv::inherited().unwrap();
        e.set(b"x".to_vec(), b"2+3".to_vec());
        for (s, n) in [
            ("1+2*3", 7),
            ("8-3-2", 3),
            ("0x10+010", 24),
            ("0 && 1/0", 0),
            ("1?4:1/0", 4),
            ("x*2", 10),
            ("9223372036854775807+1", i64::MIN),
            ("1,2", 2),
        ] {
            assert_eq!(eval(s.as_bytes(), &mut e).unwrap(), n, "{s}");
        }
        for s in ["1/0", "08", "(", "1=2"] {
            assert!(eval(s.as_bytes(), &mut e).is_err());
        }
    }
    #[test]
    fn arithmetic_limits_and_wrapping() {
        let mut e = ShellEnv::inherited().unwrap();
        e.set(b"x".to_vec(), b"x".to_vec());
        assert!(eval(b"x", &mut e).is_err());
        let nested = format!("{}1{}", "(".repeat(1000), ")".repeat(1000));
        assert!(eval(nested.as_bytes(), &mut e).is_err());
        let chain = "1+".repeat(1000) + "1";
        assert!(eval(chain.as_bytes(), &mut e).is_err());
        assert_eq!(
            eval(b"(-9223372036854775807-1)/-1", &mut e).unwrap(),
            i64::MIN
        );
        assert_eq!(eval(b"1<<64", &mut e).unwrap(), 1);
        assert_eq!(eval(b"0 && (a=1)", &mut e).unwrap(), 0);
        assert!(e.get(b"a").is_none());
        assert_eq!(eval(b"a=b=3", &mut e).unwrap(), 3);
        assert_eq!(e.get(b"a"), Some(b"3".as_slice()));
    }
    #[test]
    fn arbitrary_bytes_terminate() {
        let mut e = ShellEnv::inherited().unwrap();
        let mut seed = 11u64;
        for len in 0..512 {
            let mut input = Vec::new();
            for _ in 0..len % 128 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                input.push((seed >> 32) as u8);
            }
            let _ = eval(&input, &mut e);
        }
    }
}
