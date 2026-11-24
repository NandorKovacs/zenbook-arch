//! Files handed over by systemd (`OpenFile=` in the unit). systemd opens the
//! devices as root and passes them on, so the service itself needs no access
//! to them. See sd_listen_fds(3).

use std::fs::File;
use std::os::fd::{FromRawFd, RawFd};

/// The first passed descriptor (`SD_LISTEN_FDS_START`).
const FIRST_FD: RawFd = 3;

/// Names of the descriptors passed to this process, in descriptor order.
/// Empty when the variables are missing or meant for another process.
fn passed_names(
    pid: Option<&str>,
    count: Option<&str>,
    names: Option<&str>,
    own_pid: u32,
) -> Vec<String> {
    if pid.and_then(|p| p.parse::<u32>().ok()) != Some(own_pid) {
        return Vec::new();
    }
    let count: usize = count.and_then(|c| c.parse().ok()).unwrap_or(0);
    let mut names: Vec<String> = names.unwrap_or("").split(':').map(str::to_string).collect();
    names.resize(count, String::new());
    names
}

pub struct PassedFiles {
    names: Vec<String>,
}

impl PassedFiles {
    /// Reads `LISTEN_PID`, `LISTEN_FDS` and `LISTEN_FDNAMES`. Call this once:
    /// each descriptor can be taken only once.
    pub fn from_env() -> Self {
        let var = |name| std::env::var(name).ok();
        let names = passed_names(
            var("LISTEN_PID").as_deref(),
            var("LISTEN_FDS").as_deref(),
            var("LISTEN_FDNAMES").as_deref(),
            std::process::id(),
        );
        PassedFiles { names }
    }

    /// The passed file with this name, if there is one.
    pub fn take(&mut self, name: &str) -> Option<File> {
        let index = self.names.iter().position(|n| n == name)?;
        self.names[index].clear();
        // SAFETY: systemd passed this descriptor to this process, and
        // clearing its name above makes sure it is wrapped only once.
        Some(unsafe { File::from_raw_fd(FIRST_FD + index as RawFd) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_listed_in_descriptor_order() {
        let names = passed_names(Some("42"), Some("2"), Some("hidraw:uinput"), 42);
        assert_eq!(names, ["hidraw", "uinput"]);
    }

    #[test]
    fn variables_for_another_process_are_ignored() {
        assert!(passed_names(Some("41"), Some("2"), Some("hidraw:uinput"), 42).is_empty());
        assert!(passed_names(None, Some("2"), Some("hidraw:uinput"), 42).is_empty());
        assert!(passed_names(Some("x"), Some("2"), Some("hidraw:uinput"), 42).is_empty());
    }

    #[test]
    fn count_limits_the_names() {
        assert_eq!(
            passed_names(Some("42"), Some("1"), Some("hidraw:uinput"), 42),
            ["hidraw"]
        );
        assert_eq!(
            passed_names(Some("42"), Some("2"), Some("hidraw"), 42),
            ["hidraw", ""]
        );
        assert!(passed_names(Some("42"), None, Some("hidraw"), 42).is_empty());
    }

    #[test]
    fn take_returns_nothing_for_an_unknown_name() {
        let mut passed = PassedFiles { names: vec![] };
        assert!(passed.take("hidraw").is_none());
    }
}
