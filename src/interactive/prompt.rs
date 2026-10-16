use std::{
    borrow::Cow,
    sync::{Arc, Mutex},
};
#[derive(Clone)]
pub struct ShellPrompt {
    pub left: String,
    pub secondary: Arc<Mutex<String>>,
    pub failed: bool,
}
impl reedline::Prompt for ShellPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.left)
    }
    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }
    fn render_prompt_indicator(&self, _: reedline::PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed("")
    }
    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Owned(
            self.secondary
                .lock()
                .map(|s| s.clone())
                .unwrap_or_else(|_| "> ".to_string()),
        )
    }
    fn render_prompt_history_search_indicator(
        &self,
        s: reedline::PromptHistorySearch,
    ) -> Cow<'_, str> {
        Cow::Owned(format!("(reverse-search: {}) ", s.term))
    }
    fn get_prompt_color(&self) -> reedline::Color {
        if self.failed {
            reedline::Color::Red
        } else {
            reedline::Color::Reset
        }
    }
    fn get_indicator_color(&self) -> reedline::Color {
        reedline::Color::Reset
    }
}
pub fn default_prompt(cwd: &std::path::Path, home: Option<&[u8]>, root: bool, git: bool) -> String {
    use std::os::unix::ffi::OsStrExt;
    let shown = if let Some(home) = home.filter(|s| !s.is_empty()) {
        let home = std::path::Path::new(std::ffi::OsStr::from_bytes(home));
        if let Ok(rest) = cwd.strip_prefix(home) {
            if rest.as_os_str().is_empty() {
                "~".to_string()
            } else {
                format!("~/{}", rest.display())
            }
        } else {
            cwd.display().to_string()
        }
    } else {
        cwd.display().to_string()
    };
    let branch = if git {
        branch(cwd).map(|s| format!(" ({s})")).unwrap_or_default()
    } else {
        String::new()
    };
    format!("{shown}{branch}{} ", if root { "#" } else { "$" })
}
pub fn branch(cwd: &std::path::Path) -> Option<String> {
    use std::io::Read;
    for parent in cwd.ancestors() {
        let git = parent.join(".git");
        let git = if git.is_file() {
            let text = std::fs::read_to_string(git).ok()?;
            parent.join(text.trim().strip_prefix("gitdir: ")?)
        } else {
            git
        };
        if let Ok(f) = std::fs::File::open(git.join("HEAD")) {
            let mut text = String::new();
            f.take(4096).read_to_string(&mut text).ok()?;
            let head = text.trim();
            return Some(
                head.strip_prefix("ref: refs/heads/")
                    .unwrap_or_else(|| head.get(..8).unwrap_or(head))
                    .to_string(),
            );
        }
    }
    None
}
