//! Execution-time expansion. M3 supports only quote removal and basic variable values.
use crate::ast::{GlobChar, ParamExpr, Word, WordPart};
use crate::env::ShellEnv;

#[derive(Debug, thiserror::Error)]
#[error("rsh: expansion: {0} (full expansion is TODO(M5))")]
pub struct ExpandError(pub &'static str);

pub fn basic_word(word: &Word, env: &ShellEnv) -> Result<Vec<u8>, ExpandError> {
    let mut out = Vec::new();
    append(&word.0, env, &mut out)?;
    Ok(out)
}

fn append(parts: &[WordPart], env: &ShellEnv, out: &mut Vec<u8>) -> Result<(), ExpandError> {
    for part in parts {
        match part {
            WordPart::Literal(bytes) | WordPart::SingleQuoted(bytes) => {
                out.extend_from_slice(bytes)
            }
            WordPart::Escaped(byte) => out.push(*byte),
            WordPart::DoubleQuoted(parts) => append(parts, env, out)?,
            WordPart::Param(ParamExpr::Special(b'?')) => {
                out.extend_from_slice(env.last_status.to_string().as_bytes())
            }
            WordPart::Param(ParamExpr::Name(name)) => {
                out.extend_from_slice(env.get(name).unwrap_or_default())
            }
            WordPart::Param(ParamExpr::Braced(name)) if crate::env::valid_name(name) => {
                out.extend_from_slice(env.get(name).unwrap_or_default())
            }
            WordPart::Param(ParamExpr::Braced(name)) if name == b"?" => {
                out.extend_from_slice(env.last_status.to_string().as_bytes())
            }
            // Keep M3 literal pattern/tilde spelling; pathname and tilde expansion are M5.
            WordPart::Glob(glob) => out.push(match glob {
                GlobChar::Star => b'*',
                GlobChar::Question => b'?',
                GlobChar::OpenBracket => b'[',
            }),
            WordPart::Tilde(user) => {
                out.push(b'~');
                if let Some(user) = user {
                    out.extend_from_slice(user);
                }
            }
            WordPart::Param(_) | WordPart::CommandSub(_) | WordPart::Arith(_) => {
                return Err(ExpandError("unsupported expansion"));
            }
        }
    }
    Ok(())
}
