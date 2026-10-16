//! Evaluate conditional syntax without field splitting or pathname expansion.
use super::*;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
impl Shell {
    pub(super) fn eval_conditional(&mut self, expr: &Conditional) -> Result<Flow, ExecError> {
        if !self.env.options.double_brackets {
            sys::write_fd(2, b"rsh: [[: double_brackets option is disabled\n")?;
            return Ok(Flow::Status(2));
        }
        match self.conditional_value(expr) {
            Ok(value) => Ok(Flow::Status(u8::from(!value))),
            Err(ExecError::Unsupported("invalid extended regular expression")) => {
                Ok(Flow::Status(2))
            }
            Err(ExecError::Unsupported(reason)) => {
                sys::write_fd(2, format!("rsh: [[: {reason}\n").as_bytes())?;
                Ok(Flow::Status(1))
            }
            Err(error) => Err(error),
        }
    }
    fn conditional_value(&mut self, expr: &Conditional) -> Result<bool, ExecError> {
        Ok(match expr {
            Conditional::Not(expr) => !self.conditional_value(expr)?,
            Conditional::And(a, b) => self.conditional_value(a)? && self.conditional_value(b)?,
            Conditional::Or(a, b) => self.conditional_value(a)? || self.conditional_value(b)?,
            Conditional::Word(word) => !self
                .expand_scalar(word, crate::expand::Mode::Scalar)?
                .is_empty(),
            Conditional::Unary { op, word } => {
                let value = self.expand_scalar(word, crate::expand::Mode::Scalar)?;
                self.conditional_unary(op, &value)?
            }
            Conditional::Binary { left, op, right } => {
                let left = self.expand_scalar(left, crate::expand::Mode::Scalar)?;
                if matches!(op.as_slice(), b"=" | b"==" | b"!=") {
                    let pattern = self.case_pattern(right)?;
                    crate::expand::pattern::matches_case(
                        &pattern,
                        &left,
                        self.env.options.nocasematch,
                    ) != (op == b"!=")
                } else if op == b"=~" {
                    let executable = self.executable.clone();
                    let floor = self.fd_floor;
                    let pattern =
                        crate::expand::regex_pattern(right, &mut self.env, &mut |source, env| {
                            let mut shell = Shell::new(env.clone(), executable.clone());
                            shell.fd_floor = floor;
                            shell
                                .substitute(source)
                                .map_err(|e| crate::expand::ExpandError::new(e.to_string()))
                        });
                    if let Err(error) = &pattern {
                        self.env.exit_requested = Some(error.status);
                        self.abort_line |= error.abort_line;
                    }
                    let pattern = pattern?;
                    match sys::regex_matches(&pattern, &left, self.env.options.nocasematch) {
                        Ok(value) => value,
                        Err(_) => {
                            return Err(ExecError::Unsupported(
                                "invalid extended regular expression",
                            ));
                        }
                    }
                } else {
                    let right = self.expand_scalar(right, crate::expand::Mode::Scalar)?;
                    self.conditional_binary(&left, op, &right)?
                }
            }
        })
    }
    fn conditional_unary(&self, op: &[u8], value: &[u8]) -> Result<bool, ExecError> {
        let path = self.env.cwd.join(std::ffi::OsStr::from_bytes(value));
        let metadata = (!value.is_empty())
            .then(|| std::fs::metadata(path))
            .and_then(Result::ok);
        Ok(match op {
            b"-v" => self.env.get(value).is_some(),
            b"-R" => false, // No nameref variables exist in rsh.
            b"-o" => std::str::from_utf8(value)
                .ok()
                .and_then(|v| self.env.option(v))
                .unwrap_or(false),
            b"-a" => metadata.is_some(),
            b"-b" => metadata.is_some_and(|m| m.file_type().is_block_device()),
            b"-c" => metadata.is_some_and(|m| m.file_type().is_char_device()),
            b"-p" => metadata.is_some_and(|m| m.file_type().is_fifo()),
            b"-S" => metadata.is_some_and(|m| m.file_type().is_socket()),
            b"-u" => metadata.is_some_and(|m| m.mode() & 0o4000 != 0),
            b"-g" => metadata.is_some_and(|m| m.mode() & 0o2000 != 0),
            b"-k" => metadata.is_some_and(|m| m.mode() & 0o1000 != 0),
            b"-O" => metadata.is_some_and(|m| m.uid() == sys::effective_uid()),
            b"-G" => metadata.is_some_and(|m| m.gid() == sys::effective_gid()),
            b"-N" => {
                metadata.is_some_and(|m| (m.mtime(), m.mtime_nsec()) > (m.atime(), m.atime_nsec()))
            }
            _ => builtin_impl::test::unary(op, value, &self.env).map_err(ExecError::Unsupported)?,
        })
    }
    fn conditional_binary(&mut self, a: &[u8], op: &[u8], b: &[u8]) -> Result<bool, ExecError> {
        Ok(match op {
            b"<" => a < b,
            b">" => a > b,
            b"-nt" | b"-ot" | b"-ef" => {
                let metadata = |value: &[u8]| {
                    if value.is_empty() {
                        None
                    } else {
                        std::fs::metadata(self.env.cwd.join(std::ffi::OsStr::from_bytes(value)))
                            .ok()
                    }
                };
                let left = metadata(a);
                let right = metadata(b);
                match (op, left, right) {
                    (b"-ef", Some(a), Some(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
                    (b"-nt", Some(a), Some(b)) => {
                        (a.mtime(), a.mtime_nsec()) > (b.mtime(), b.mtime_nsec())
                    }
                    (b"-ot", Some(a), Some(b)) => {
                        (a.mtime(), a.mtime_nsec()) < (b.mtime(), b.mtime_nsec())
                    }
                    (b"-nt", Some(_), None) | (b"-ot", None, Some(_)) => true,
                    _ => false,
                }
            }
            _ => {
                let left = crate::expand::arith::eval(a, &mut self.env)
                    .map_err(|_| ExecError::Unsupported("invalid arithmetic operand"))?;
                let right = crate::expand::arith::eval(b, &mut self.env)
                    .map_err(|_| ExecError::Unsupported("invalid arithmetic operand"))?;
                match op {
                    b"-eq" => left == right,
                    b"-ne" => left != right,
                    b"-lt" => left < right,
                    b"-le" => left <= right,
                    b"-gt" => left > right,
                    b"-ge" => left >= right,
                    _ => return Err(ExecError::Unsupported("unknown conditional operator")),
                }
            }
        })
    }
}
