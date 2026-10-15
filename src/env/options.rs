//! Shell options shared by startup parsing and the set builtin.
use super::ShellEnv;
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub errexit: bool,
    pub nounset: bool,
    pub xtrace: bool,
    pub noexec: bool,
    pub pipefail: bool,
    pub interactive: bool,
    pub login: bool,
    pub expand_aliases: bool,
}
pub const NAMES: &[&str] = &[
    "errexit",
    "nounset",
    "xtrace",
    "noglob",
    "noexec",
    "noclobber",
    "pipefail",
    "expand_aliases",
];
impl ShellEnv {
    pub fn option(&self, name: &str) -> Option<bool> {
        Some(match name {
            "errexit" => self.options.errexit,
            "nounset" => self.options.nounset,
            "xtrace" => self.options.xtrace,
            "noglob" => self.noglob,
            "noexec" => self.options.noexec,
            "noclobber" => self.noclobber,
            "pipefail" => self.options.pipefail,
            "expand_aliases" => self.options.expand_aliases,
            _ => return None,
        })
    }
    pub fn set_option(&mut self, name: &str, value: bool) -> bool {
        match name {
            "errexit" => self.options.errexit = value,
            "nounset" => self.options.nounset = value,
            "xtrace" => self.options.xtrace = value,
            "noglob" => self.noglob = value,
            "noexec" => self.options.noexec = value,
            "noclobber" => self.noclobber = value,
            "pipefail" => self.options.pipefail = value,
            "expand_aliases" => self.options.expand_aliases = value,
            _ => return false,
        }
        true
    }
    pub fn set_flag(&mut self, flag: u8, value: bool) -> bool {
        let name = match flag {
            b'e' => "errexit",
            b'u' => "nounset",
            b'x' => "xtrace",
            b'f' => "noglob",
            b'n' => "noexec",
            b'C' => "noclobber",
            b'i' => {
                self.options.interactive = value;
                return true;
            }
            b'l' => {
                self.options.login = value;
                return true;
            }
            _ => return false,
        };
        self.set_option(name, value)
    }
    pub fn option_letters(&self) -> Vec<u8> {
        let mut s = Vec::new();
        for (c, on) in [
            (b'e', self.options.errexit),
            (b'u', self.options.nounset),
            (b'x', self.options.xtrace),
            (b'f', self.noglob),
            (b'n', self.options.noexec),
            (b'C', self.noclobber),
            (b'i', self.options.interactive),
        ] {
            if on {
                s.push(c);
            }
        }
        s
    }
}
