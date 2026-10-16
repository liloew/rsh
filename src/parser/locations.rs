//! Preserve diagnostic source lines across canonical internal worker serialization.
use crate::ast::*;
fn visit(script: &mut Script, action: &mut impl FnMut(&mut usize)) {
    fn compound(c: &mut CompoundCommand, a: &mut impl FnMut(&mut usize)) {
        match c {
            CompoundCommand::Conditional(_) => {}
            CompoundCommand::Brace(s)
            | CompoundCommand::Subshell(s)
            | CompoundCommand::For { body: s, .. } => visit(s, a),
            CompoundCommand::While { condition, body }
            | CompoundCommand::Until { condition, body } => {
                visit(condition, a);
                visit(body, a);
            }
            CompoundCommand::If {
                branches,
                else_body,
            } => {
                for b in branches {
                    visit(&mut b.condition, a);
                    visit(&mut b.body, a);
                }
                if let Some(b) = else_body {
                    visit(b, a);
                }
            }
            CompoundCommand::Case { items, .. } => {
                for i in items {
                    visit(&mut i.body, a);
                }
            }
        }
    }
    for i in &mut script.0 {
        for p in
            std::iter::once(&mut i.and_or.first).chain(i.and_or.rest.iter_mut().map(|(_, p)| p))
        {
            for c in &mut p.commands {
                match c {
                    Command::Simple(s) => action(&mut s.line),
                    Command::Compound(c, _) => compound(c, action),
                    Command::FunctionDef { definition, .. } => {
                        compound(&mut definition.body, action)
                    }
                }
            }
        }
    }
}
pub fn lines(script: &Script) -> Vec<u8> {
    let mut clone = script.clone();
    let mut out = Vec::new();
    visit(&mut clone, &mut |n| out.extend((*n as u64).to_le_bytes()));
    out
}
pub fn restore(script: &mut Script, bytes: &[u8]) {
    let mut chunks = bytes.as_chunks::<8>().0.iter();
    visit(script, &mut |n| {
        if let Some(b) = chunks.next()
            && let Ok(v) = usize::try_from(u64::from_le_bytes(*b))
        {
            *n = v;
        }
    });
}
