//! The arguments after `lgtm agent review <command>`.

pub struct Args(Vec<String>);

impl Args {
    pub fn new(args: Vec<String>) -> Self {
        Self(args)
    }

    /// Takes the command word, when the first argument isn't a flag.
    pub fn next_word(&mut self) -> Option<String> {
        let first = self.0.first()?;
        (!first.starts_with('-') || first == "-h" || first == "--help").then(|| self.0.remove(0))
    }

    /// Takes `flag`, returning whether it was there.
    pub fn flag(&mut self, flag: &str) -> bool {
        let before = self.0.len();
        self.0.retain(|a| a != flag);
        self.0.len() != before
    }

    /// Takes `flag` and the value after it.
    pub fn value(&mut self, flag: &str) -> Result<Option<String>, String> {
        let Some(i) = self.0.iter().position(|a| a == flag) else {
            return Ok(None);
        };
        if i + 1 >= self.0.len() {
            return Err(format!("{flag} needs a value"));
        }
        let value = self.0.remove(i + 1);
        self.0.remove(i);
        Ok(Some(value))
    }

    /// What's left, refusing flags nothing took.
    pub fn rest(&mut self) -> Result<Vec<String>, String> {
        if let Some(f) = self.0.iter().find(|a| a.starts_with("--")) {
            return Err(format!("unknown flag `{f}`"));
        }
        Ok(std::mem::take(&mut self.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Args {
        Args::new(s.split(' ').map(String::from).collect())
    }

    #[test]
    fn flags_and_values_come_out_anywhere_and_leftover_flags_are_refused() {
        let mut a = args("mark a.rs --removed 3-4 1,5 --flagged");
        assert_eq!(a.next_word().as_deref(), Some("mark"));
        assert_eq!(a.value("--removed").unwrap().as_deref(), Some("3-4"));
        assert!(a.flag("--flagged"));
        assert!(!a.flag("--clear"));
        assert_eq!(a.rest().unwrap(), ["a.rs", "1,5"]);

        assert!(args("check --bogus a.rs").rest().is_err());
        assert!(args("comment a.rs:3 --as").value("--as").is_err());
        assert_eq!(args("--uncheck").next_word(), None);
    }
}
