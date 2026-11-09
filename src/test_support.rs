//! Helpers for unit tests.

use std::os::unix::fs::DirBuilderExt;
use std::process::Command;

/// Set in the child process that [`in_temp_home`] starts.
const CHILD: &str = "HUSH_TEST_TEMP_HOME";

const FOLDERS: [(&str, &str); 5] = [
    ("XDG_CONFIG_HOME", ".config"),
    ("XDG_DATA_HOME", ".local/share"),
    ("XDG_CACHE_HOME", ".cache"),
    ("XDG_STATE_HOME", ".local/state"),
    ("XDG_RUNTIME_DIR", "run"),
];

/// Put `in_temp_home!(test_name);` first in a test to run the test in a
/// child process whose `HOME` and XDG folders are in a new temporary folder,
/// so that it cannot read or write the real user folders. The parent returns
/// when the child passes, and fails when the child fails.
///
/// Only the child gets the changed environment. The parent never calls
/// `set_var`, so the tests that run beside it in one process (`cargo test`)
/// cannot see a change, and nothing races.
macro_rules! in_temp_home {
    ($test:ident) => {
        if !$crate::test_support::run_in_temp_home(concat!(module_path!(), "::", stringify!($test)))
        {
            return;
        }
    };
}
pub(crate) use in_temp_home;

/// `true` in the child that runs `test`, `false` in the parent after the
/// child passes.
pub fn run_in_temp_home(test: &str) -> bool {
    if std::env::var_os(CHILD).is_some() {
        return true;
    }
    let name = test.split_once("::").map_or(test, |(_, rest)| rest);
    let home = tempfile::tempdir().expect("create the temporary home");
    let binary = std::env::current_exe().expect("find the test binary");
    let mut command = Command::new(binary);
    command
        .args(["--exact", name, "--test-threads=1"])
        .env(CHILD, "1")
        .env("HOME", home.path())
        .env_remove(crate::config::paths::CONFIG_ENV);
    for (var, folder) in FOLDERS {
        let path = home.path().join(folder);
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&path)
            .expect("create the XDG folder");
        command.env(var, path);
    }
    let output = command.output().expect("run the test in a child process");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // A name that matches no test runs nothing, and the child still exits 0.
    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "{name} failed in a temporary home:\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    false
}
