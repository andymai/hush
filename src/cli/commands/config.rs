//! `hush config export|import`: move the settings and the vocabulary to
//! another machine.

use super::daemon::{handle_daemon_restart, SessionOptions};
use crate::cli::ConfigCommands;
use crate::config::transfer::{self, ImportReport, Locations, Mode};
use crate::ipc::client;
use anyhow::{Context, Result};
use std::path::Path;

pub async fn handle_config(command: ConfigCommands) -> Result<()> {
    let locations = Locations::current();
    match command {
        ConfigCommands::Export { file, force } => {
            transfer::export(&locations, &file, force)?;
            println!(
                "Wrote the settings and the vocabulary to {}.",
                file.display()
            );
            println!(
                "The file has no API keys. Put them in ~/.config/hush/.env on the other machine."
            );
            Ok(())
        },
        ConfigCommands::Import { file, merge } => {
            let mode = if merge { Mode::Merge } else { Mode::Replace };
            let report = transfer::import(&locations, &file, mode)?;
            for line in report_lines(&report, mode, &file) {
                println!("{}", line);
            }
            if report.written.is_empty() || !client::is_running().await {
                return Ok(());
            }
            println!("Restarting the daemon to apply the change.");
            handle_daemon_restart(SessionOptions {
                editing_mode: "light".to_string(),
                no_processing: false,
                no_button: false,
            })
            .await
            .context("The import is done, but the daemon did not restart. Run: hush daemon restart")
        },
    }
}

fn report_lines(report: &ImportReport, mode: Mode, file: &Path) -> Vec<String> {
    let mut lines = Vec::new();
    if mode == Mode::Merge {
        if report.written.is_empty() {
            lines.push(format!(
                "Nothing to add: the settings and the vocabulary already have every key in {}.",
                file.display()
            ));
            return lines;
        }
        lines.push(format!(
            "Added {} and {}.",
            count(report.settings_added, "setting", "settings"),
            count(
                report.vocabulary_added,
                "vocabulary entry",
                "vocabulary entries"
            )
        ));
    }
    for (path, backup) in &report.written {
        lines.push(match backup {
            Some(backup) => format!(
                "Wrote {}. The old file is {}.",
                path.display(),
                backup.display()
            ),
            None => format!("Wrote {}.", path.display()),
        });
    }
    lines
}

fn count(n: usize, one: &str, many: &str) -> String {
    format!("{} {}", n, if n == 1 { one } else { many })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn the_report_names_each_file_and_its_backup() {
        let file = Path::new("hush.toml");
        let report = ImportReport {
            settings_added: 1,
            vocabulary_added: 3,
            written: vec![
                (
                    PathBuf::from("/c/config.toml"),
                    Some(PathBuf::from("/c/config.toml.bak")),
                ),
                (PathBuf::from("/c/vocabulary.json"), None),
            ],
        };
        assert_eq!(
            report_lines(&report, Mode::Merge, file),
            [
                "Added 1 setting and 3 vocabulary entries.",
                "Wrote /c/config.toml. The old file is /c/config.toml.bak.",
                "Wrote /c/vocabulary.json.",
            ]
        );
        assert_eq!(report_lines(&report, Mode::Replace, file).len(), 2);
        let nothing = report_lines(&ImportReport::default(), Mode::Merge, file);
        assert!(nothing[0].starts_with("Nothing to add"), "{nothing:?}");
    }
}
