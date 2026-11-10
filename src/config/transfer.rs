//! `hush config export` and `hush config import`: the user's settings and
//! vocabulary in one TOML file that another machine can read.
//!
//! The file holds only what `config.toml` and `vocabulary.json` hold. API
//! keys stay in the environment and the `.env` files, and an export that
//! contains one of them is refused.

use super::settings::{rename_legacy_keys, Config, TranscriptionBackend};
use super::{keys, paths, server};
use crate::text_processing::DomainVocabulary;
use anyhow::{bail, Context, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

pub const FORMAT: &str = "hush-export";
pub const VERSION: i64 = 1;

const HEADER: &str = "# Hush settings and vocabulary. Load them with: hush config import <file>\n";

/// Shorter values are too common to mark a key.
const MIN_KEY_LEN: usize = 8;

/// The files an export reads and an import writes.
#[derive(Debug, Clone)]
pub struct Locations {
    pub config: PathBuf,
    pub vocabulary: PathBuf,
}

impl Locations {
    /// The config file and the vocabulary file that Hush uses.
    pub fn current() -> Self {
        Self {
            config: paths::config_path(),
            vocabulary: DomainVocabulary::default_path(),
        }
    }
}

/// How an import treats the current files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The imported settings and vocabulary take the place of the current ones
    Replace,
    /// Keep every current value and add only the keys and entries that are missing
    Merge,
}

/// The settings and the vocabulary of an export.
#[derive(Debug, Clone, PartialEq)]
pub struct Bundle {
    /// The user's own keys, without the compiled-in defaults
    pub config: toml::Table,
    pub vocabulary: DomainVocabulary,
}

impl Bundle {
    /// Read the current files. A missing file counts as empty.
    pub fn read(locations: &Locations) -> Result<Self> {
        Ok(Self {
            config: read_user_config(&locations.config)?,
            vocabulary: read_vocabulary(&locations.vocabulary)?,
        })
    }

    pub fn to_toml(&self) -> Result<String> {
        let vocabulary = toml::Value::try_from(&self.vocabulary)
            .context("Failed to serialize the vocabulary")?;
        let mut file = toml::Table::new();
        file.insert("format".into(), FORMAT.into());
        file.insert("version".into(), VERSION.into());
        file.insert("config".into(), toml::Value::Table(self.config.clone()));
        file.insert("vocabulary".into(), vocabulary);
        let body = toml::to_string(&file).context("Failed to serialize the export")?;
        Ok(format!("{}{}", HEADER, body))
    }

    /// Parse an export, check its format and version, and check the
    /// settings and the vocabulary the same way as a normal load.
    pub fn parse(text: &str) -> Result<Self> {
        let mut file: toml::Table = toml::from_str(text).context("The file is not valid TOML")?;
        if file.get("format").and_then(toml::Value::as_str) != Some(FORMAT) {
            bail!(
                "The file is not a Hush export: it needs format = \"{}\"",
                FORMAT
            );
        }
        match file.get("version").and_then(toml::Value::as_integer) {
            Some(VERSION) => {},
            Some(version) if version > VERSION => bail!(
                "The file has version {}, and this Hush reads only version {}. Update Hush, then try again.",
                version,
                VERSION
            ),
            Some(version) => bail!(
                "The file has version {}, and this Hush reads only version {}",
                version,
                VERSION
            ),
            None => bail!("The file has no version number"),
        }
        let config = match file.remove("config") {
            Some(toml::Value::Table(config)) => normalised(config),
            _ => bail!("The file has no [config] table"),
        };
        let vocabulary = match file.remove("vocabulary") {
            Some(value @ toml::Value::Table(_)) => value
                .try_into()
                .context("The vocabulary in the file is not valid")?,
            _ => bail!("The file has no [vocabulary] table"),
        };
        let bundle = Self { config, vocabulary };
        bundle.check()?;
        Ok(bundle)
    }

    fn check(&self) -> Result<Config> {
        Config::from_user_table(self.config.clone()).context("The settings are not valid")
    }
}

/// Write the current settings and vocabulary to `file`. An existing file
/// stays unless `force`.
pub fn export(locations: &Locations, file: &Path, force: bool) -> Result<()> {
    export_with(locations, file, force, api_keys)
}

fn export_with(
    locations: &Locations,
    file: &Path,
    force: bool,
    keys: impl FnOnce(&Config) -> Vec<(String, String)>,
) -> Result<()> {
    if !force && file.exists() {
        bail!(
            "{} already exists. Add --force to replace it.",
            file.display()
        );
    }
    let bundle = Bundle::read(locations)?;
    let config = bundle
        .check()
        .with_context(|| format!("Fix {} first", locations.config.display()))?;
    let text = bundle.to_toml()?;
    refuse_keys(&text, &keys(&config))?;
    write_atomic(file, text.as_bytes(), force)
}

/// What an import changed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ImportReport {
    /// Settings that a merge added
    pub settings_added: usize,
    /// Vocabulary entries that a merge added
    pub vocabulary_added: usize,
    /// Each file written, and its backup when the file was there before
    pub written: Vec<(PathBuf, Option<PathBuf>)>,
}

/// Import `file` into the config file and the vocabulary file. Nothing
/// changes until the whole file passes its checks.
pub fn import(locations: &Locations, file: &Path, mode: Mode) -> Result<ImportReport> {
    let text =
        fs::read_to_string(file).with_context(|| format!("Failed to read {}", file.display()))?;
    let incoming =
        Bundle::parse(&text).with_context(|| format!("Did not import {}", file.display()))?;
    let mut report = ImportReport::default();
    let (config, vocabulary) = match mode {
        Mode::Replace => (Some(incoming.config), Some(incoming.vocabulary)),
        Mode::Merge => {
            let mut current = Bundle::read(locations)?;
            report.settings_added = add_missing(&mut current.config, incoming.config);
            report.vocabulary_added = current.vocabulary.add_missing(&incoming.vocabulary);
            current
                .check()
                .context("The merged settings are not valid; nothing changed")?;
            (
                (report.settings_added > 0).then_some(current.config),
                (report.vocabulary_added > 0).then_some(current.vocabulary),
            )
        },
    };

    let mut staged = Vec::new();
    if let Some(config) = config {
        let text = toml::to_string_pretty(&config).context("Failed to serialize the settings")?;
        staged.push(Staged::new(&locations.config, text.as_bytes())?);
    }
    if let Some(vocabulary) = vocabulary {
        let json = serde_json::to_string_pretty(&vocabulary)
            .context("Failed to serialize the vocabulary")?;
        staged.push(Staged::new(&locations.vocabulary, json.as_bytes())?);
    }
    for file in staged {
        report.written.push(file.commit()?);
    }
    Ok(report)
}

/// Write `path` through a temporary file beside it and a rename, so that a
/// reader never sees half a file. An existing file stays unless `replace`.
pub fn write_atomic(path: &Path, contents: &[u8], replace: bool) -> Result<()> {
    let temp = temp_beside(path, contents)?;
    let written = if replace {
        temp.persist(path)
    } else {
        temp.persist_noclobber(path)
    };
    written
        .map_err(|e| e.error)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

/// Replace the file at `path`, or the file that a link at `path` names,
/// through [`write_atomic`], so that the daemon never reads half of it. In a
/// folder that takes no new file, such as one a configuration manager made
/// read-only around a writable file, it writes the file in place.
pub fn replace_file(path: &Path, contents: &[u8]) -> Result<()> {
    let path = link_target(path);
    if let Some(parent) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    match write_atomic(&path, contents, true) {
        Ok(()) => Ok(()),
        Err(e) if denied(&e) && path.exists() => fs::write(&path, contents)
            .with_context(|| format!("Failed to write {}", path.display())),
        Err(e) => Err(e),
    }
}

fn denied(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io| io.kind() == std::io::ErrorKind::PermissionDenied)
    })
}

/// Copy the file at `path` to the same name with `.bak` added, and return
/// the copy's path. A missing file has nothing to keep.
pub fn back_up(path: &Path) -> Result<Option<PathBuf>> {
    let path = link_target(path);
    if !path.exists() {
        return Ok(None);
    }
    let backup = backup_path(&path);
    fs::copy(&path, &backup).with_context(|| format!("Failed to back up {}", path.display()))?;
    Ok(Some(backup))
}

/// A new file, written in full beside its target and not yet in its place.
struct Staged {
    path: PathBuf,
    temp: NamedTempFile,
}

impl Staged {
    fn new(path: &Path, contents: &[u8]) -> Result<Self> {
        let path = link_target(path);
        if let Some(parent) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        Ok(Self {
            temp: temp_beside(&path, contents)?,
            path,
        })
    }

    fn commit(self) -> Result<(PathBuf, Option<PathBuf>)> {
        let backup = if self.path.exists() {
            let backup = backup_path(&self.path);
            fs::copy(&self.path, &backup)
                .with_context(|| format!("Failed to back up {}", self.path.display()))?;
            Some(backup)
        } else {
            None
        };
        self.temp
            .persist(&self.path)
            .map_err(|e| e.error)
            .with_context(|| format!("Failed to write {}", self.path.display()))?;
        Ok((self.path, backup))
    }
}

// Dotfile managers often link config.toml into a repository. A rename onto
// the link would put a plain file in its place, so write the file it names.
// A link whose file does not exist yet names where to create it.
fn link_target(path: &Path) -> PathBuf {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => fs::canonicalize(path)
            .or_else(|_| {
                fs::read_link(path).map(|target| match path.parent() {
                    Some(parent) if target.is_relative() => parent.join(target),
                    _ => target,
                })
            })
            .unwrap_or_else(|_| path.to_path_buf()),
        _ => path.to_path_buf(),
    }
}

fn temp_beside(path: &Path, contents: &[u8]) -> Result<NamedTempFile> {
    let dir = match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    };
    let mut temp = tempfile::Builder::new()
        .prefix(".hush-")
        .tempfile_in(dir)
        .with_context(|| format!("Failed to create a temporary file in {}", dir.display()))?;
    if let Ok(metadata) = fs::metadata(path) {
        temp.as_file()
            .set_permissions(metadata.permissions())
            .with_context(|| format!("Failed to copy the permissions of {}", path.display()))?;
    }
    temp.write_all(contents)
        .and_then(|()| temp.as_file().sync_all())
        .with_context(|| format!("Failed to write a temporary file in {}", dir.display()))?;
    Ok(temp)
}

fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".bak");
    path.with_file_name(name)
}

fn read_user_config(path: &Path) -> Result<toml::Table> {
    if !path.exists() {
        return Ok(toml::Table::new());
    }
    let text = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file {}", path.display()))?;
    let table =
        toml::from_str(&text).with_context(|| format!("Invalid config file {}", path.display()))?;
    Ok(normalised(table))
}

fn read_vocabulary(path: &Path) -> Result<DomainVocabulary> {
    if path.exists() {
        DomainVocabulary::load_from_file(path)
    } else {
        Ok(DomainVocabulary::default())
    }
}

fn normalised(mut table: toml::Table) -> toml::Table {
    rename_legacy_keys(&mut table);
    table
}

/// Add each key of `incoming` that `current` lacks, at any depth, and keep
/// every value `current` has; returns how many values were added.
fn add_missing(current: &mut toml::Table, incoming: toml::Table) -> usize {
    let mut added = 0;
    for (key, value) in incoming {
        match (current.get_mut(&key), value) {
            (Some(toml::Value::Table(mine)), toml::Value::Table(theirs)) => {
                added += add_missing(mine, theirs);
            },
            (Some(_), _) => {},
            (None, value) => {
                added += leaves(&value);
                current.insert(key, value);
            },
        }
    }
    added
}

fn leaves(value: &toml::Value) -> usize {
    match value {
        toml::Value::Table(table) => table.values().map(leaves).sum(),
        _ => 1,
    }
}

/// Each API key that Hush can read now, with its variable.
fn api_keys(config: &Config) -> Vec<(String, String)> {
    TranscriptionBackend::ALL
        .iter()
        .filter_map(|backend| backend.key_var())
        .chain(["ANTHROPIC_API_KEY"])
        .chain(server::key_var(&config.transcription.server_key_env))
        .chain(server::key_var(&config.llm.server_key_env))
        .filter_map(|var| keys::api_key(var).map(|key| (var.to_string(), key)))
        .collect()
}

fn refuse_keys(text: &str, keys: &[(String, String)]) -> Result<()> {
    for (var, key) in keys {
        let key = key.trim();
        if key.len() >= MIN_KEY_LEN && text.contains(key) {
            bail!(
                "The settings or the vocabulary contain the value of {}. Remove the key from \
                 them, and keep it in the environment or a .env file.",
                var
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dangling_link_stays_a_link_and_its_file_is_created() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("dotfiles").join("config.toml");
        fs::create_dir_all(real.parent().unwrap()).unwrap();
        let link = dir.path().join("config.toml");
        std::os::unix::fs::symlink("dotfiles/config.toml", &link).unwrap();
        replace_file(&link, b"new").unwrap();
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_to_string(&real).unwrap(), "new");
    }

    #[test]
    fn a_read_only_folder_still_takes_a_write_to_its_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("managed");
        fs::create_dir_all(&folder).unwrap();
        let file = folder.join("config.toml");
        fs::write(&file, "old").unwrap();
        fs::set_permissions(&folder, fs::Permissions::from_mode(0o555)).unwrap();
        let written = replace_file(&file, b"new");
        fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).unwrap();
        written.unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "new");
    }

    #[test]
    fn a_replaced_file_keeps_its_link_and_a_backup_keeps_the_old_text() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("dotfiles").join("config.toml");
        fs::create_dir_all(real.parent().unwrap()).unwrap();
        fs::write(&real, "old").unwrap();
        let link = dir.path().join("config.toml");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let backup = back_up(&link).unwrap().unwrap();
        assert_eq!(backup, real.with_file_name("config.toml.bak"));
        assert_eq!(fs::read_to_string(&backup).unwrap(), "old");

        replace_file(&link, b"new").unwrap();
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_to_string(&real).unwrap(), "new");

        let fresh = dir.path().join("sub").join("vocabulary.json");
        assert_eq!(back_up(&fresh).unwrap(), None);
        replace_file(&fresh, b"{}").unwrap();
        assert_eq!(fs::read_to_string(&fresh).unwrap(), "{}");
    }

    struct Machine {
        _dir: tempfile::TempDir,
        root: PathBuf,
        locations: Locations,
    }

    fn machine() -> Machine {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let locations = Locations {
            config: root.join("hush/config.toml"),
            vocabulary: root.join("hush/vocabulary.json"),
        };
        Machine {
            _dir: dir,
            root,
            locations,
        }
    }

    fn export_here(locations: &Locations, file: &Path, force: bool) -> Result<()> {
        export_with(locations, file, force, |_| Vec::new())
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn set_up(machine: &Machine, config: &str, terms: &[&str]) {
        write(&machine.locations.config, config);
        let mut vocabulary = DomainVocabulary::default();
        for term in terms {
            crate::text_processing::learn::learn_into(&mut vocabulary, term);
        }
        vocabulary.add_abbreviation("k8s".into(), "Kubernetes".into());
        vocabulary
            .save_to_file(&machine.locations.vocabulary)
            .unwrap();
    }

    fn contents(machine: &Machine) -> (String, String) {
        (
            fs::read_to_string(&machine.locations.config).unwrap(),
            fs::read_to_string(&machine.locations.vocabulary).unwrap(),
        )
    }

    const SETTINGS: &str = "[hotkey]\ncombination = \"F12\"\nmode = \"toggle\"\n\n\
        [insertion]\nuinput_apps = [\"konsole\"]\n\n\
        [insertion.paste_keys]\nkitty = \"ctrl+shift+v\"\n";

    #[test]
    fn export_then_import_gives_the_same_settings_and_vocabulary() {
        let source = machine();
        set_up(&source, SETTINGS, &["Kubernetes", "PostgreSQL"]);
        let file = source.root.join("hush-export.toml");
        export_here(&source.locations, &file, false).unwrap();

        let text = fs::read_to_string(&file).unwrap();
        assert!(text.contains("format = \"hush-export\""), "{text}");
        assert!(text.contains("version = 1"), "{text}");
        assert!(
            !text.contains("sample_rate"),
            "only the user's keys, not the defaults: {text}"
        );

        let target = machine();
        let report = import(&target.locations, &file, Mode::Replace).unwrap();
        assert_eq!(report.written.len(), 2);
        assert!(report.written.iter().all(|(_, backup)| backup.is_none()));

        assert_eq!(
            Bundle::read(&target.locations).unwrap(),
            Bundle::read(&source.locations).unwrap()
        );
        assert_eq!(
            Config::load_from_file(&target.locations.config).unwrap(),
            Config::load_from_file(&source.locations.config).unwrap()
        );
    }

    #[test]
    fn an_empty_setup_round_trips() {
        let source = machine();
        let file = source.root.join("empty.toml");
        export_here(&source.locations, &file, false).unwrap();
        let bundle = Bundle::parse(&fs::read_to_string(&file).unwrap()).unwrap();
        assert!(bundle.config.is_empty());
        assert_eq!(bundle.vocabulary, DomainVocabulary::default());
    }

    #[test]
    fn export_refuses_to_overwrite_without_force() {
        let source = machine();
        set_up(&source, SETTINGS, &["Kubernetes"]);
        let file = source.root.join("hush-export.toml");
        fs::write(&file, "keep me").unwrap();

        let err = export_here(&source.locations, &file, false).unwrap_err();
        assert!(err.to_string().contains("--force"), "{err}");
        assert_eq!(fs::read_to_string(&file).unwrap(), "keep me");

        export_here(&source.locations, &file, true).unwrap();
        assert!(Bundle::parse(&fs::read_to_string(&file).unwrap()).is_ok());
    }

    #[test]
    fn a_wrong_header_or_version_changes_nothing() {
        let source = machine();
        set_up(&source, SETTINGS, &["Kubernetes"]);
        let good = source.root.join("good.toml");
        export_here(&source.locations, &good, false).unwrap();
        let text = fs::read_to_string(&good).unwrap();

        let target = machine();
        set_up(&target, "[hotkey]\ncombination = \"F9\"\n", &["Hush"]);
        let before = contents(&target);
        for (name, bad, expected) in [
            (
                "format",
                text.replace("hush-export", "other-app"),
                "not a Hush export",
            ),
            (
                "no-format",
                text.replace("format = \"hush-export\"", ""),
                "not a Hush export",
            ),
            (
                "newer",
                text.replace("version = 1", "version = 2"),
                "version 2",
            ),
            (
                "older",
                text.replace("version = 1", "version = 0"),
                "version 0",
            ),
            ("no-version", text.replace("version = 1", ""), "no version"),
            (
                "settings",
                text.replace("\"F12\"", "\"Ctrl+Bogus\""),
                "settings are not valid",
            ),
            ("not-toml", "format = ".to_string(), "not valid TOML"),
        ] {
            let file = target.root.join(format!("{name}.toml"));
            fs::write(&file, bad).unwrap();
            for mode in [Mode::Replace, Mode::Merge] {
                let err = format!("{:#}", import(&target.locations, &file, mode).unwrap_err());
                assert!(err.contains(expected), "{name}: {err}");
                assert_eq!(contents(&target), before, "{name}");
            }
        }
        assert!(!backup_path(&target.locations.config).exists());
        assert!(!backup_path(&target.locations.vocabulary).exists());
    }

    #[test]
    fn replace_keeps_a_backup_of_each_file() {
        let source = machine();
        set_up(&source, SETTINGS, &["Kubernetes"]);
        let file = source.root.join("hush-export.toml");
        export_here(&source.locations, &file, false).unwrap();

        let target = machine();
        set_up(&target, "[hotkey]\ncombination = \"F9\"\n", &["Hush"]);
        let (config, vocabulary) = contents(&target);
        let report = import(&target.locations, &file, Mode::Replace).unwrap();

        let config_backup = backup_path(&target.locations.config);
        let vocabulary_backup = backup_path(&target.locations.vocabulary);
        assert!(config_backup.ends_with("config.toml.bak"));
        assert_eq!(
            report.written,
            [
                (target.locations.config.clone(), Some(config_backup.clone())),
                (
                    target.locations.vocabulary.clone(),
                    Some(vocabulary_backup.clone())
                ),
            ]
        );
        assert_eq!(fs::read_to_string(config_backup).unwrap(), config);
        assert_eq!(fs::read_to_string(vocabulary_backup).unwrap(), vocabulary);
        let imported = Bundle::read(&target.locations).unwrap().vocabulary;
        assert!(!imported.technical_terms.contains_key("hush"));
    }

    #[test]
    fn merge_keeps_current_values_and_adds_only_missing_keys() {
        let source = machine();
        set_up(
            &source,
            "[hotkey]\ncombination = \"F12\"\nmode = \"toggle\"\n\n[audio]\npause_media = true\n",
            &["KUBERNETES", "PostgreSQL"],
        );
        let file = source.root.join("hush-export.toml");
        export_here(&source.locations, &file, false).unwrap();

        let target = machine();
        set_up(&target, "[hotkey]\ncombination = \"F9\"\n", &["Kubernetes"]);
        let report = import(&target.locations, &file, Mode::Merge).unwrap();
        assert_eq!(report.settings_added, 2);
        assert_eq!(report.vocabulary_added, 1);

        let config = Config::load_from_file(&target.locations.config).unwrap();
        assert_eq!(config.hotkey.combination, "F9", "the current value stays");
        assert_eq!(config.hotkey.mode, crate::hotkey::HotkeyMode::Toggle);
        assert!(config.audio.pause_media);
        let vocabulary = Bundle::read(&target.locations).unwrap().vocabulary;
        assert_eq!(vocabulary.technical_terms["kubernetes"], "Kubernetes");
        assert_eq!(vocabulary.technical_terms["postgresql"], "PostgreSQL");
        assert!(backup_path(&target.locations.config).exists());

        let before = contents(&target);
        let again = import(&target.locations, &file, Mode::Merge).unwrap();
        assert_eq!(again, ImportReport::default(), "nothing left to add");
        assert_eq!(contents(&target), before);
    }

    #[test]
    fn a_linked_config_stays_a_link() {
        let source = machine();
        set_up(&source, SETTINGS, &["Kubernetes"]);
        let file = source.root.join("hush-export.toml");
        export_here(&source.locations, &file, false).unwrap();

        let target = machine();
        let dotfiles = target.root.join("dotfiles/config.toml");
        write(&dotfiles, "[hotkey]\ncombination = \"F9\"\n");
        fs::create_dir_all(target.locations.config.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&dotfiles, &target.locations.config).unwrap();

        import(&target.locations, &file, Mode::Replace).unwrap();
        let link = fs::symlink_metadata(&target.locations.config).unwrap();
        assert!(link.file_type().is_symlink());
        assert_eq!(
            Config::load_from_file(&dotfiles)
                .unwrap()
                .hotkey
                .combination,
            "F12"
        );
        assert!(backup_path(&dotfiles).exists());
    }

    #[test]
    fn merge_into_an_empty_machine_writes_both_files() {
        let source = machine();
        set_up(&source, SETTINGS, &["Kubernetes"]);
        let file = source.root.join("hush-export.toml");
        export_here(&source.locations, &file, false).unwrap();

        let target = machine();
        let report = import(&target.locations, &file, Mode::Merge).unwrap();
        assert_eq!(report.written.len(), 2);
        assert_eq!(
            Bundle::read(&target.locations).unwrap(),
            Bundle::read(&source.locations).unwrap()
        );
    }

    #[test]
    fn an_export_with_an_api_key_is_refused() {
        let keys = [(
            "GROQ_API_KEY".to_string(),
            "gsk_0123456789abcdef".to_string(),
        )];
        let err = refuse_keys("[hooks]\ntranscript = \"x gsk_0123456789abcdef\"\n", &keys)
            .unwrap_err()
            .to_string();
        assert!(err.contains("GROQ_API_KEY"), "{err}");
        assert!(!err.contains("gsk_"), "the message never shows the key");
        assert!(refuse_keys("[hotkey]\ncombination = \"F12\"\n", &keys).is_ok());
        let short = [("X_KEY".to_string(), "F12".to_string())];
        assert!(refuse_keys("combination = \"F12\"", &short).is_ok());

        let source = machine();
        set_up(
            &source,
            "[hooks]\ntranscript = \"tool --token gsk_0123456789abcdef\"\n",
            &[],
        );
        let file = source.root.join("hush-export.toml");
        let err = export_with(&source.locations, &file, false, |_| keys.to_vec()).unwrap_err();
        assert!(err.to_string().contains("GROQ_API_KEY"), "{err}");
        assert!(!file.exists(), "nothing is written");
    }

    #[test]
    fn add_missing_counts_each_new_value() {
        let mut current: toml::Table = toml::from_str("[a]\nx = 1\n").unwrap();
        let incoming: toml::Table =
            toml::from_str("[a]\nx = 2\ny = 3\n[b]\nz = 4\n[b.c]\nw = 5\n").unwrap();
        assert_eq!(add_missing(&mut current, incoming), 3);
        assert_eq!(current["a"]["x"].as_integer(), Some(1));
        assert_eq!(current["b"]["c"]["w"].as_integer(), Some(5));
    }
}
