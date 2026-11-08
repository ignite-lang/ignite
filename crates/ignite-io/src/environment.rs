//! The capability `Environment` (`docs/slice.md` 21.20): the environment
//! variables of the process, read once at launch by the generated `main`.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::sync::Arc;

use crate::IoError;

/// The environment variables of the executable, a snapshot taken at
/// launch, or the entries of `Environment.of(..)` in a test. A cheap
/// clone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Environment {
    variables: Arc<BTreeMap<String, String>>,
    _counted: Arc<crate::census::Counted>,
}

impl Environment {
    /// `Environment.of(entries)`, test-only: `NAME=value` entries, as
    /// `Invocation.environment` writes them, split at the first `=`. The
    /// last entry of a name wins; an entry without `=` or with an empty
    /// name is skipped.
    pub fn of(entries: Vec<String>) -> Environment {
        let variables = entries
            .into_iter()
            .filter_map(|entry| {
                let (name, value) = entry.split_once('=')?;
                (!name.is_empty()).then(|| (name.to_string(), value.to_string()))
            })
            .collect();
        Environment {
            variables: Arc::new(variables),
            _counted: Arc::new(crate::census::Counted::new(
                crate::census::Kind::Capabilities,
            )),
        }
    }

    /// The resource `Environment` at launch: the process environment.
    /// Never fails; the `Result` keeps the launch of every capability one
    /// shape.
    pub fn launch() -> Result<Environment, IoError> {
        Ok(Environment::from_os_vars(std::env::vars_os()))
    }

    /// `vars` as an environment. A variable whose name or value is not
    /// UTF-8 is skipped: it cannot be a `Text`, and refusing it would end
    /// a program over a variable it never reads.
    pub fn from_os_vars(vars: impl Iterator<Item = (OsString, OsString)>) -> Environment {
        let variables = vars
            .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
            .collect();
        Environment {
            variables: Arc::new(variables),
            _counted: Arc::new(crate::census::Counted::new(
                crate::census::Kind::Capabilities,
            )),
        }
    }

    /// `environment.get(name)`: the value of the variable `name`, `None`
    /// when it is not set.
    pub fn get(&self, name: impl AsRef<str>) -> Option<String> {
        self.variables.get(name.as_ref()).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::Environment;
    use std::ffi::{OsStr, OsString};
    use std::os::unix::ffi::OsStrExt;

    /// `Environment.of`: `NAME=value` entries, split at the first `=`; the
    /// last entry of a name wins; an entry without `=` or with an empty
    /// name is skipped.
    #[test]
    fn of_reads_name_value_entries() {
        let environment = Environment::of(
            ["A=1", "EMPTY=", "B=c=d", "A=2", "NO_EQUALS", "=x"]
                .map(String::from)
                .to_vec(),
        );
        assert_eq!(environment.get("A"), Some("2".to_string()));
        assert_eq!(environment.get("EMPTY"), Some(String::new()));
        assert_eq!(environment.get("B"), Some("c=d".to_string()));
        assert_eq!(environment.get("NO_EQUALS"), None);
        assert_eq!(environment.get(""), None);
        assert_eq!(environment.get("UNSET"), None);
    }

    /// The launch keeps every variable whose name and value are UTF-8 and
    /// skips the others, so one odd variable does not end the program.
    #[test]
    fn launch_skips_variables_that_are_not_utf8() {
        let vars = [
            (OsString::from("SESSION_STORE"), OsString::from("redis")),
            (
                OsString::from("ODD_VALUE"),
                OsStr::from_bytes(b"a\xffb").to_os_string(),
            ),
            (
                OsStr::from_bytes(b"ODD\xffNAME").to_os_string(),
                OsString::from("x"),
            ),
            (OsString::from("EMPTY"), OsString::new()),
        ];
        let environment = Environment::from_os_vars(vars.into_iter());
        assert_eq!(environment.get("SESSION_STORE"), Some("redis".to_string()));
        assert_eq!(environment.get("ODD_VALUE"), None);
        assert_eq!(environment.get("EMPTY"), Some(String::new()));
    }

    /// The resource at launch holds the process environment.
    #[test]
    fn launch_reads_the_process_environment() {
        let environment = Environment::launch().unwrap();
        let path = std::env::var("PATH").ok();
        assert_eq!(environment.get("PATH"), path);
    }
}
