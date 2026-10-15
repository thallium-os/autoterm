/// Commands that only read state. Only these are run without asking;
/// everything else (rm, dd, sudo, git, find, unknown tools) needs Enter.
/// Deny-by-default means a new or unfamiliar command is always confirmed.
/// Only the name is classified, never the arguments, so nothing here may be
/// able to run another program or change state through a flag: that rules
/// out env (`env rm -rf ~`), fd (`-x`), rg (`--pre`) and history (`clear`).
const SILENT: &[&str] = &[
    "bat", "cal", "cat", "cd", "clear", "date", "df", "diff", "du", "echo", "file", "free", "head",
    "htop", "btop", "id", "la", "less", "ll", "ls", "man", "more", "printenv", "ps", "pwd", "grep",
    "stat", "tail", "tldr", "top", "tree", "type", "uptime", "wc", "whereis", "which", "whoami",
];

#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    Silent,
    Confirm,
}

impl Mode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Mode::Silent => "silent",
            Mode::Confirm => "confirm",
        }
    }
}

pub fn classify(cmd: &str) -> Mode {
    if SILENT.contains(&cmd) {
        Mode::Silent
    } else {
        Mode::Confirm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destructive_and_privileged_need_confirmation() {
        for c in [
            "rm",
            "dd",
            "sudo",
            "doas",
            "chmod",
            "mkfs",
            "find",
            "git",
            "never-heard-of-it",
        ] {
            assert_eq!(classify(c), Mode::Confirm, "{c}");
        }
    }

    #[test]
    fn commands_that_run_other_commands_need_confirmation() {
        for c in [
            "env", "fd", "rg", "history", "xargs", "nohup", "time", "watch",
        ] {
            assert_eq!(classify(c), Mode::Confirm, "{c}");
        }
    }

    #[test]
    fn read_only_is_silent() {
        for c in ["ls", "cd", "grep", "cat"] {
            assert_eq!(classify(c), Mode::Silent, "{c}");
        }
    }
}
