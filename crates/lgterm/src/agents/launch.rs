//! Starting an agent with the user's command for it, through their interactive shell, so an
//! alias or function from their shell's startup files works the way it does at their prompt.

/// The program and arguments that run `command` with `args` through `shell` (zsh when unset).
pub fn argv(command: &str, args: &[String], shell: &str) -> Vec<String> {
    let mut line = command.trim().to_owned();
    for a in args {
        line.push(' ');
        line.push_str(&quote(a));
    }
    let shell = if shell.is_empty() { "/bin/zsh" } else { shell };
    vec![shell.into(), "-i".into(), "-c".into(), line]
}

/// `s` in single quotes, for sh, bash, zsh and fish alike.
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_stays_open_to_aliases_and_the_arguments_stay_whole() {
        let args = ["fix it's tests".to_owned(), "$HOME".to_owned()];
        assert_eq!(
            argv("cc", &args, "/bin/bash"),
            ["/bin/bash", "-i", "-c", r"cc 'fix it'\''s tests' '$HOME'"]
        );
        assert_eq!(argv("codex", &[], "")[0], "/bin/zsh");
    }
}
