//! Shell options shared by startup parsing and the set builtin.
use super::ShellEnv;
/// Shell-local settings; named options are also carried into re-exec workers.
#[derive(Debug, Clone)]
pub struct Options {
    pub here_strings: bool,
    pub double_brackets: bool,
    pub dotglob: bool,
    pub nullglob: bool,
    pub failglob: bool,
    pub nocasematch: bool,
    pub sourcepath: bool,
    pub vi: bool,
    pub git_prompt: bool,
    pub monitor: bool,
    pub errexit: bool,
    pub nounset: bool,
    pub xtrace: bool,
    pub noexec: bool,
    pub pipefail: bool,
    pub interactive: bool,
    pub login: bool,
    pub expand_aliases: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            // Explicit compatibility extensions default on; their gates affect
            // execution, not lexer/parser recognition or strict POSIX parsing.
            here_strings: true,
            double_brackets: true,
            dotglob: false,
            nullglob: false,
            failglob: false,
            nocasematch: false,
            // Bare source/dot operands use cwd unless the user opts into PATH lookup.
            sourcepath: false,
            vi: false,
            git_prompt: false,
            monitor: false,
            errexit: false,
            nounset: false,
            xtrace: false,
            noexec: false,
            pipefail: false,
            interactive: false,
            login: false,
            expand_aliases: true,
        }
    }
}
/// Keep this registry in sync with option/set_option: worker snapshots iterate it.
pub const NAMES: &[&str] = &[
    "here_strings",
    "double_brackets",
    "dotglob",
    "nullglob",
    "failglob",
    "nocasematch",
    "sourcepath",
    "vi",
    "git_prompt",
    "monitor",
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
            "here_strings" => self.options.here_strings,
            "double_brackets" => self.options.double_brackets,
            "dotglob" => self.options.dotglob,
            "nullglob" => self.options.nullglob,
            "failglob" => self.options.failglob,
            "nocasematch" => self.options.nocasematch,
            "sourcepath" => self.options.sourcepath,

            "vi" => self.options.vi,
            "git_prompt" => self.options.git_prompt,
            "monitor" => self.options.monitor,
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
            "here_strings" => self.options.here_strings = value,
            "double_brackets" => self.options.double_brackets = value,
            "dotglob" => self.options.dotglob = value,
            "nullglob" => self.options.nullglob = value,
            "failglob" => self.options.failglob = value,
            "nocasematch" => self.options.nocasematch = value,
            "sourcepath" => self.options.sourcepath = value,

            "vi" => self.options.vi = value,
            "git_prompt" => self.options.git_prompt = value,
            "monitor" => self.options.monitor = value,
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
            b'm' => "monitor",
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
            (b'm', self.options.monitor),
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
