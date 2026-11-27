//! API keys live in the environment, the Secret Service, or
//! `~/.config/hush/.env`, never in config.toml, so the config file is safe to
//! share. A variable in the environment wins, then the Secret Service, then
//! the `.env` file.
//!
//! In the Flatpak, the sandbox's own `.env` takes the place of the Secret
//! Service. Hush never asks the Secret portal: its unlock prompt could take
//! the keyboard focus while Hush types, and the text would go into the
//! password field.
//!
//! The daemon never unlocks the keyring. It reads only the unlocked items,
//! into a cache, and a locked item sends one notice. Only the settings
//! window unlocks the keyring, through [`store`] and [`remove`].

use super::paths;
use super::settings::{Config, TranscriptionBackend};
use anyhow::{bail, Context, Result};
use parking_lot::{Condvar, Mutex};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ffi::OsString;
use std::fmt;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tracing::debug;

static LOADED: Mutex<BTreeSet<OsString>> = Mutex::new(BTreeSet::new());

/// How long a caller that needs a key waits for the first read of the
/// keyring, before it goes without.
const FILL_WAIT: Duration = Duration::from_secs(2);

/// The attributes of each item that Hush keeps in the Secret Service.
pub(crate) const APPLICATION: (&str, &str) = ("application", "hush");
pub(crate) const VARIABLE: &str = "variable";

/// Where a key comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Environment,
    Keyring,
    /// An item in the keyring, which is locked, so the daemon cannot read it
    Locked,
    EnvFile,
}

impl Source {
    /// For `hush doctor` and `hush status`: where the key is, never the key.
    pub fn describe(self) -> &'static str {
        match self {
            Source::Environment => "from the environment",
            Source::Keyring => "from the keyring",
            Source::Locked => "in the locked keyring",
            Source::EnvFile => "from a .env file",
        }
    }
}

/// What a search of the keyring found for a variable. A search reads no
/// secret, so it never unlocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Found {
    Absent,
    Locked,
    Unlocked,
}

/// The values of the unlocked items of Hush, by variable, and whether a
/// locked item exists.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Unlocked {
    pub values: HashMap<String, String>,
    pub locked: bool,
}

/// The Secret Service, behind a trait so that the tests use a double.
pub(crate) trait SecretStore: Send + Sync {
    fn find(&self, var: &str) -> Result<Found>;
    /// Never unlocks: a locked item only sets [`Unlocked::locked`].
    fn read_unlocked(&self) -> Result<Unlocked>;
    /// Create or replace the item of `var`. This may unlock the keyring.
    fn store(&self, var: &str, value: &str) -> Result<()>;
    /// Delete each item of `var`. This may unlock the keyring.
    fn remove(&self, var: &str) -> Result<bool>;
}

/// The tests' keyring, which holds nothing.
#[cfg(test)]
struct NoKeyring;

#[cfg(test)]
impl SecretStore for NoKeyring {
    fn find(&self, _var: &str) -> Result<Found> {
        Ok(Found::Absent)
    }

    fn read_unlocked(&self) -> Result<Unlocked> {
        Ok(Unlocked::default())
    }

    fn store(&self, _var: &str, _value: &str) -> Result<()> {
        bail!("The tests have no keyring")
    }

    fn remove(&self, _var: &str) -> Result<bool> {
        Ok(false)
    }
}

/// The keys of the unlocked items, read once on a thread of their own. A
/// caller that comes before the read ends waits at most [`FILL_WAIT`].
pub(crate) struct KeyCache {
    store: Arc<dyn SecretStore>,
    notice: Box<dyn Fn() + Send + Sync>,
    state: Mutex<CacheState>,
    filled: Condvar,
}

#[derive(Default)]
struct CacheState {
    started: bool,
    filled: bool,
    values: HashMap<String, String>,
    locked: bool,
    noticed: bool,
}

impl KeyCache {
    pub(crate) fn new(store: Arc<dyn SecretStore>, notice: Box<dyn Fn() + Send + Sync>) -> Self {
        Self {
            store,
            notice,
            state: Mutex::new(CacheState::default()),
            filled: Condvar::new(),
        }
    }

    /// Start the first read, once.
    pub(crate) fn fill(self: &Arc<Self>) {
        {
            let mut state = self.state.lock();
            if state.started {
                return;
            }
            state.started = true;
        }
        let cache = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name("hush-keys".into())
            .spawn(move || cache.read());
        if let Err(e) = spawned {
            debug!("Could not read the keyring: {}", e);
            self.take(Unlocked::default());
        }
    }

    fn read(&self) {
        let unlocked = self.store.read_unlocked().unwrap_or_else(|e| {
            debug!("No keys from the keyring: {:#}", e);
            Unlocked::default()
        });
        self.take(unlocked);
    }

    fn take(&self, unlocked: Unlocked) {
        let notice = {
            let mut state = self.state.lock();
            state.values = unlocked.values;
            state.locked = unlocked.locked;
            state.filled = true;
            let notice = state.locked && !state.noticed;
            state.noticed |= notice;
            notice
        };
        self.filled.notify_all();
        if notice {
            (self.notice)();
        }
    }

    /// The key of `var` from the unlocked items. While a locked item
    /// exists, each miss reads the keyring again, so a key comes back
    /// after the unlock.
    pub(crate) fn get(self: &Arc<Self>, var: &str) -> Option<String> {
        self.fill();
        let deadline = Instant::now() + FILL_WAIT;
        let mut state = self.state.lock();
        while !state.filled {
            if self.filled.wait_until(&mut state, deadline).timed_out() {
                return None;
            }
        }
        if let Some(value) = state.values.get(var) {
            return Some(value.clone());
        }
        if !state.locked {
            return None;
        }
        drop(state);
        self.read();
        self.state.lock().values.get(var).cloned()
    }
}

struct Keys {
    /// The Secret Service, or `None` in the Flatpak
    keyring: Option<Arc<dyn SecretStore>>,
    cache: Option<Arc<KeyCache>>,
}

fn keys() -> &'static Keys {
    static KEYS: OnceLock<Keys> = OnceLock::new();
    KEYS.get_or_init(|| {
        let keyring = native_keyring();
        let cache = keyring
            .clone()
            .map(|store| Arc::new(KeyCache::new(store, Box::new(locked_notice))));
        Keys { keyring, cache }
    })
}

#[cfg(all(target_os = "linux", not(test)))]
fn native_keyring() -> Option<Arc<dyn SecretStore>> {
    if crate::portal::sandboxed() {
        None
    } else {
        Some(Arc::new(super::secret_service::SecretService))
    }
}

#[cfg(test)]
fn native_keyring() -> Option<Arc<dyn SecretStore>> {
    Some(Arc::new(NoKeyring))
}

fn locked_notice() {
    crate::notify::notify(
        crate::notify::NoticeKind::Problem,
        "The keyring is locked",
        "Hush works without the API keys in the keyring until you unlock it. \
         Hush settings unlocks it when you save a key.",
    );
}

/// Each variable that `config` can take a key from: the hosted speech
/// services', Anthropic's, and the two servers', once each.
pub fn variables(config: &Config) -> Vec<String> {
    let mut variables: Vec<String> = Vec::new();
    let named = TranscriptionBackend::ALL
        .iter()
        .filter_map(|backend| backend.key_var())
        .chain(["ANTHROPIC_API_KEY"])
        .chain(super::server::key_var(&config.transcription.server_key_env))
        .chain(super::server::key_var(&config.llm.server_key_env));
    for var in named {
        if !variables.iter().any(|known| known == var) {
            variables.push(var.to_string());
        }
    }
    variables
}

/// `~/.config/hush/.env`
pub fn env_file() -> PathBuf {
    paths::config_dir().join(".env")
}

/// Load `~/.config/hush/.env` (installed daemons) and `./.env` (source
/// checkouts) into the environment. A variable that is already set wins.
pub fn load_env_files() {
    load_with(|| {
        let _ = dotenvy::from_path(env_file());
        let _ = dotenvy::dotenv();
    });
}

/// The variables that [`load_env_files`] added, so that a child process
/// can go without them.
pub fn loaded_from_env_files() -> Vec<OsString> {
    LOADED.lock().iter().cloned().collect()
}

// dotenvy never overrides a set variable, so each name that is new after the
// load came from a file and not from the user's session.
pub(crate) fn load_with(load: impl FnOnce()) {
    let before: HashSet<OsString> = std::env::vars_os().map(|(name, _)| name).collect();
    load();
    LOADED.lock().extend(
        std::env::vars_os()
            .map(|(name, _)| name)
            .filter(|name| !before.contains(name)),
    );
}

/// Start to read the unlocked keys of the keyring on a thread of their
/// own, so the first key that a take needs is ready. The daemon calls this
/// at startup. Without it, the first [`api_key`] starts the read.
pub fn fill_cache() {
    if let Some(cache) = &keys().cache {
        cache.fill();
    }
}

/// Whether Hush has a key in `var`: in the environment, in the keyring
/// (locked or not), or in `~/.config/hush/.env`. The keyring part is a
/// search, which reads no secret and never unlocks.
pub fn present(var: &str) -> bool {
    source(var).is_some()
}

/// Where the key in `var` comes from.
pub fn source(var: &str) -> Option<Source> {
    source_in(&env_file(), var)
}

/// [`source`] with `env_file` in place of `~/.config/hush/.env`.
pub fn source_in(env_file: &Path, var: &str) -> Option<Source> {
    source_with(keys().keyring.as_deref(), env_file, var)
}

fn source_with(keyring: Option<&dyn SecretStore>, env_file: &Path, var: &str) -> Option<Source> {
    if from_session(var).is_some() {
        return Some(Source::Environment);
    }
    let found = keyring.map_or(Found::Absent, |keyring| {
        keyring.find(var).unwrap_or_else(|e| {
            debug!("Could not search the keyring for {}: {:#}", var, e);
            Found::Absent
        })
    });
    if found == Found::Unlocked {
        return Some(Source::Keyring);
    }
    if from_file(env_file, var).is_some() || from_loaded(var).is_some() {
        return Some(Source::EnvFile);
    }
    (found == Found::Locked).then_some(Source::Locked)
}

/// The key in `var`, from the environment, else from the unlocked keyring,
/// else from `~/.config/hush/.env`, when it is set and not blank. The file
/// is read each time, so a key added while a window is open is found.
pub fn api_key(var: &str) -> Option<String> {
    api_key_in(&env_file(), var)
}

/// [`api_key`] with `env_file` in place of `~/.config/hush/.env`.
pub fn api_key_in(env_file: &Path, var: &str) -> Option<String> {
    api_key_with(keys().cache.as_ref(), env_file, var)
}

fn api_key_with(cache: Option<&Arc<KeyCache>>, env_file: &Path, var: &str) -> Option<String> {
    from_session(var)
        .or_else(|| cache.and_then(|cache| cache.get(var)))
        .or_else(|| from_file(env_file, var))
        .or_else(|| from_loaded(var))
}

/// Keep `value` as the key in `var`: in the keyring, or in the Flatpak in
/// the sandbox's `.env`. The keyring may ask the user to unlock it, so only
/// the settings window calls this.
pub fn store(var: &str, value: &str) -> Result<()> {
    store_in(keys().keyring.as_deref(), &env_file(), var, value)
}

fn store_in(
    keyring: Option<&dyn SecretStore>,
    env_file: &Path,
    var: &str,
    value: &str,
) -> Result<()> {
    check_name(var)?;
    let value = value.trim();
    if value.is_empty() {
        bail!("The key for {} is empty", var);
    }
    match keyring {
        Some(keyring) => keyring
            .store(var, value)
            .with_context(|| format!("Could not save {} in the keyring", var)),
        None => write_env_line(env_file, var, Some(value)).map(drop),
    }
}

/// Delete the key in `var` from the keyring and from `~/.config/hush/.env`.
/// The keyring may ask the user to unlock it, so only the settings window
/// calls this.
pub fn remove(var: &str) -> Result<()> {
    remove_in(keys().keyring.as_deref(), &env_file(), var)
}

fn remove_in(keyring: Option<&dyn SecretStore>, env_file: &Path, var: &str) -> Result<()> {
    check_name(var)?;
    if let Some(keyring) = keyring {
        keyring
            .remove(var)
            .with_context(|| format!("Could not remove {} from the keyring", var))?;
    }
    write_env_line(env_file, var, None).map(drop)
}

fn check_name(var: &str) -> Result<()> {
    if !super::server::valid_var_name(var) {
        bail!("{} is not the name of an environment variable", var);
    }
    Ok(())
}

/// An API key that a request reads when it goes out. A key from a variable
/// is read for each request, so a key that a locked keyring held at startup
/// reaches the requests after the unlock.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiKey {
    /// A value that the caller gave
    Value(String),
    /// The variable that [`api_key_in`] reads, with this `.env` file
    Var { var: String, env_file: PathBuf },
}

impl ApiKey {
    pub fn var(var: &str, env_file: &Path) -> Self {
        Self::Var {
            var: var.to_string(),
            env_file: env_file.to_path_buf(),
        }
    }

    /// A given value that is empty or only spaces.
    pub fn is_blank(&self) -> bool {
        matches!(self, Self::Value(value) if value.trim().is_empty())
    }

    /// The key now, or an error that names its variable and never shows a
    /// value.
    pub async fn require(&self) -> Result<String> {
        self.read().await.with_context(|| match self {
            Self::Value(_) => "The API key is empty".to_string(),
            Self::Var { var, .. } => format!(
                "Hush cannot read {} now. Unlock the keyring, or save the key again in Hush \
                 settings",
                var
            ),
        })
    }

    /// The key now: the value, or what [`api_key_in`] finds. The keyring
    /// read waits off the async threads.
    async fn read(&self) -> Option<String> {
        match self {
            Self::Value(value) => Some(value.clone()).filter(|value| !value.trim().is_empty()),
            Self::Var { var, env_file } => {
                let (var, env_file) = (var.clone(), env_file.clone());
                tokio::task::spawn_blocking(move || api_key_in(&env_file, &var))
                    .await
                    .ok()
                    .flatten()
            },
        }
    }
}

impl From<String> for ApiKey {
    fn from(value: String) -> Self {
        Self::Value(value)
    }
}

impl From<&str> for ApiKey {
    fn from(value: &str) -> Self {
        Self::Value(value.to_string())
    }
}

/// Never shows a value, so a provider in a log line keeps its key.
impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(_) => f.write_str("ApiKey(..)"),
            Self::Var { var, .. } => write!(f, "ApiKey({})", var),
        }
    }
}

/// A variable of the user's session. A variable that a `.env` file loaded
/// comes after the keyring, so it is not one.
fn from_session(var: &str) -> Option<String> {
    if LOADED.lock().contains(&OsString::from(var)) {
        return None;
    }
    std::env::var(var).ok().filter(|key| !key.trim().is_empty())
}

/// A variable that [`load_env_files`] loaded, as from `./.env`.
fn from_loaded(var: &str) -> Option<String> {
    if !LOADED.lock().contains(&OsString::from(var)) {
        return None;
    }
    std::env::var(var).ok().filter(|key| !key.trim().is_empty())
}

fn from_file(path: &Path, var: &str) -> Option<String> {
    dotenvy::from_path_iter(path)
        .ok()?
        .filter_map(Result::ok)
        .find(|(name, _)| name == var)
        .map(|(_, value)| value)
        .filter(|key| !key.trim().is_empty())
}

/// Set `var` to `value` in the `.env` file at `path`, or drop its line with
/// `None`. Each other line stays as it is. The new text goes to a temporary
/// file in the same folder with mode 0600, which then replaces the file.
/// Returns whether the file changed.
pub(crate) fn write_env_line(path: &Path, var: &str, value: Option<&str>) -> Result<bool> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| format!("Failed to read {}", path.display())),
    };
    let line = value.map(|value| env_line(var, value)).transpose()?;
    let mut out = String::with_capacity(text.len() + 80);
    let mut found = false;
    for raw in text.split_inclusive('\n') {
        if !assigns(raw, var) {
            out.push_str(raw);
            continue;
        }
        // The first line wins when dotenvy reads the file, so it takes the
        // new value, and later lines of the same variable go.
        if let (false, Some(line)) = (found, &line) {
            out.push_str(line);
            out.push('\n');
        }
        found = true;
    }
    if let (false, Some(line)) = (found, &line) {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(line);
        out.push('\n');
    }
    if out == text {
        return Ok(false);
    }
    write_private(path, out.as_bytes())?;
    Ok(true)
}

/// Single quotes, because dotenvy expands `$` in other values.
fn env_line(var: &str, value: &str) -> Result<String> {
    if value.chars().any(|c| c.is_control() || c == '\'') {
        bail!(
            "The key for {} has a quote mark or a line break, so it cannot go in a .env file",
            var
        );
    }
    Ok(format!("{}='{}'", var, value))
}

fn assigns(line: &str, var: &str) -> bool {
    let line = line.trim_start();
    let line = line
        .strip_prefix("export ")
        .map_or(line, |rest| rest.trim_start());
    line.strip_prefix(var)
        .is_some_and(|rest| rest.trim_start().starts_with('='))
}

fn write_private(path: &Path, contents: &[u8]) -> Result<()> {
    let dir = match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    };
    std::fs::create_dir_all(dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    let mut temp = tempfile::Builder::new()
        .prefix(".hush-")
        .tempfile_in(dir)
        .with_context(|| format!("Failed to create a temporary file in {}", dir.display()))?;
    temp.as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o600))
        .and_then(|()| temp.write_all(contents))
        .and_then(|()| temp.as_file().sync_all())
        .with_context(|| format!("Failed to write a temporary file in {}", dir.display()))?;
    temp.persist(path)
        .map_err(|e| e.error)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;

    #[test]
    fn a_key_is_read_from_an_env_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        std::fs::write(
            &path,
            "# keys\nGROQ_API_KEY=gsk_test\nOPENAI_API_KEY=\"  \"\n",
        )
        .unwrap();
        assert_eq!(
            from_file(&path, "GROQ_API_KEY").as_deref(),
            Some("gsk_test")
        );
        assert_eq!(from_file(&path, "OPENAI_API_KEY"), None, "blank is unset");
        assert_eq!(from_file(&path, "ANTHROPIC_API_KEY"), None);
        assert_eq!(from_file(&dir.path().join("missing"), "GROQ_API_KEY"), None);
    }

    #[test]
    fn only_variables_from_a_file_are_recorded() {
        std::env::set_var("HUSH_TEST_KEYS_SESSION", "from-session");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        std::fs::write(
            &path,
            "HUSH_TEST_KEYS_FILE=secret\nHUSH_TEST_KEYS_SESSION=from-file\n",
        )
        .unwrap();
        load_with(|| {
            let _ = dotenvy::from_path(&path);
        });
        assert_eq!(
            std::env::var("HUSH_TEST_KEYS_FILE").as_deref(),
            Ok("secret")
        );
        assert_eq!(
            std::env::var("HUSH_TEST_KEYS_SESSION").as_deref(),
            Ok("from-session"),
            "the session wins"
        );
        let loaded = loaded_from_env_files();
        assert!(loaded.contains(&OsString::from("HUSH_TEST_KEYS_FILE")));
        assert!(!loaded.contains(&OsString::from("HUSH_TEST_KEYS_SESSION")));
    }

    /// A keyring double: what a search finds, what a read gives, and each
    /// call it got.
    #[derive(Default)]
    struct FakeKeyring {
        found: Mutex<HashMap<String, Found>>,
        unlocked: Mutex<Unlocked>,
        reads: AtomicUsize,
        /// A read waits for a message on this channel
        gate: Mutex<Option<mpsc::Receiver<()>>>,
        stored: Mutex<Vec<(String, String)>>,
        removed: Mutex<Vec<String>>,
    }

    impl FakeKeyring {
        fn holding(var: &str, value: &str) -> Self {
            let fake = Self::default();
            fake.found.lock().insert(var.to_string(), Found::Unlocked);
            fake.unlocked
                .lock()
                .values
                .insert(var.to_string(), value.to_string());
            fake
        }

        fn locked(var: &str) -> Self {
            let fake = Self::default();
            fake.found.lock().insert(var.to_string(), Found::Locked);
            fake.unlocked.lock().locked = true;
            fake
        }
    }

    impl SecretStore for FakeKeyring {
        fn find(&self, var: &str) -> Result<Found> {
            Ok(self.found.lock().get(var).copied().unwrap_or(Found::Absent))
        }

        fn read_unlocked(&self) -> Result<Unlocked> {
            if let Some(gate) = self.gate.lock().as_ref() {
                let _ = gate.recv();
            }
            self.reads.fetch_add(1, Ordering::SeqCst);
            Ok(self.unlocked.lock().clone())
        }

        fn store(&self, var: &str, value: &str) -> Result<()> {
            self.stored
                .lock()
                .push((var.to_string(), value.to_string()));
            Ok(())
        }

        fn remove(&self, var: &str) -> Result<bool> {
            self.removed.lock().push(var.to_string());
            Ok(true)
        }
    }

    fn cache_over(fake: Arc<FakeKeyring>, notices: Arc<AtomicUsize>) -> Arc<KeyCache> {
        Arc::new(KeyCache::new(
            fake,
            Box::new(move || {
                notices.fetch_add(1, Ordering::SeqCst);
            }),
        ))
    }

    #[test]
    fn the_environment_wins_then_the_keyring_then_the_env_file() {
        let var = "HUSH_TEST_KEYS_ORDER";
        let dir = tempfile::tempdir().unwrap();
        let env_file = dir.path().join(".env");
        std::fs::write(&env_file, format!("{}=test-key-file\n", var)).unwrap();
        let fake = Arc::new(FakeKeyring::holding(var, "test-key-keyring"));
        let cache = cache_over(Arc::clone(&fake), Arc::default());

        assert_eq!(
            source_with(Some(fake.as_ref()), &env_file, var),
            Some(Source::Keyring)
        );
        assert_eq!(
            api_key_with(Some(&cache), &env_file, var).as_deref(),
            Some("test-key-keyring"),
            "the keyring comes before the .env file"
        );
        assert_eq!(
            source_with(None, &env_file, var),
            Some(Source::EnvFile),
            "the Flatpak has no keyring"
        );
        assert_eq!(
            api_key_with(None, &env_file, var).as_deref(),
            Some("test-key-file")
        );

        std::env::set_var(var, "test-key-session");
        assert_eq!(
            source_with(Some(fake.as_ref()), &env_file, var),
            Some(Source::Environment)
        );
        assert_eq!(
            api_key_with(Some(&cache), &env_file, var).as_deref(),
            Some("test-key-session"),
            "the environment wins over both"
        );
        std::env::remove_var(var);

        let empty = Arc::new(FakeKeyring::default());
        let none = dir.path().join("missing.env");
        assert_eq!(source_with(Some(empty.as_ref()), &none, var), None);
        assert_eq!(
            api_key_with(Some(&cache_over(empty, Arc::default())), &none, var),
            None
        );
    }

    #[test]
    fn a_variable_from_a_dot_env_comes_after_the_keyring() {
        let var = "HUSH_TEST_KEYS_LOADED";
        let dir = tempfile::tempdir().unwrap();
        let local = dir.path().join(".env");
        std::fs::write(&local, format!("{}=test-key-local\n", var)).unwrap();
        load_with(|| {
            let _ = dotenvy::from_path(&local);
        });
        let other = dir.path().join("other.env");
        let fake = Arc::new(FakeKeyring::holding(var, "test-key-keyring"));
        assert_eq!(
            api_key_with(Some(&cache_over(fake, Arc::default())), &other, var).as_deref(),
            Some("test-key-keyring")
        );
        assert_eq!(
            source_with(None, &other, var),
            Some(Source::EnvFile),
            "a variable that ./.env loaded is a .env key"
        );
        assert_eq!(
            api_key_with(None, &other, var).as_deref(),
            Some("test-key-local")
        );
    }

    #[test]
    fn a_locked_item_counts_as_present_but_gives_no_key() {
        let var = "HUSH_TEST_KEYS_LOCKED";
        let dir = tempfile::tempdir().unwrap();
        let env_file = dir.path().join(".env");
        let fake = Arc::new(FakeKeyring::locked(var));
        assert_eq!(
            source_with(Some(fake.as_ref()), &env_file, var),
            Some(Source::Locked)
        );
        let notices = Arc::new(AtomicUsize::new(0));
        let cache = cache_over(Arc::clone(&fake), Arc::clone(&notices));
        assert_eq!(api_key_with(Some(&cache), &env_file, var), None);
        assert_eq!(api_key_with(Some(&cache), &env_file, var), None);
        assert_eq!(notices.load(Ordering::SeqCst), 1, "one notice");
        assert_eq!(
            fake.reads.load(Ordering::SeqCst),
            3,
            "the fill, then one read again for each miss"
        );

        std::fs::write(&env_file, format!("{}=test-key-file\n", var)).unwrap();
        assert_eq!(
            source_with(Some(fake.as_ref()), &env_file, var),
            Some(Source::EnvFile),
            "a key that Hush can read wins over the locked one"
        );
        std::fs::remove_file(&env_file).unwrap();

        {
            let mut unlocked = fake.unlocked.lock();
            unlocked.locked = false;
            unlocked
                .values
                .insert(var.to_string(), "test-key-1".to_string());
        }
        assert_eq!(
            api_key_with(Some(&cache), &env_file, var).as_deref(),
            Some("test-key-1"),
            "a later read finds the key after the unlock"
        );
        assert_eq!(notices.load(Ordering::SeqCst), 1);
        let reads = fake.reads.load(Ordering::SeqCst);
        assert_eq!(cache.get(var).as_deref(), Some("test-key-1"));
        assert_eq!(fake.reads.load(Ordering::SeqCst), reads, "from the cache");
    }

    #[test]
    fn a_caller_waits_at_most_two_seconds_for_the_first_read() {
        let var = "HUSH_TEST_KEYS_SLOW";
        let fake = Arc::new(FakeKeyring::holding(var, "test-key-1"));
        let (open, gate) = mpsc::channel();
        *fake.gate.lock() = Some(gate);
        let cache = cache_over(Arc::clone(&fake), Arc::default());

        let two_seconds = Duration::from_secs(2);
        let start = Instant::now();
        assert_eq!(cache.get(var), None, "goes without the key");
        let waited = start.elapsed();
        assert!(waited >= two_seconds, "{waited:?}");
        assert!(waited < two_seconds + Duration::from_secs(1), "{waited:?}");

        open.send(()).unwrap();
        let start = Instant::now();
        assert_eq!(cache.get(var).as_deref(), Some("test-key-1"));
        assert!(start.elapsed() < two_seconds, "the read ended");
        assert_eq!(fake.reads.load(Ordering::SeqCst), 1, "one read");
    }

    #[test]
    fn a_caller_gets_the_key_when_the_read_ends_within_the_wait() {
        let var = "HUSH_TEST_KEYS_SOON";
        let fake = Arc::new(FakeKeyring::holding(var, "test-key-2"));
        let (open, gate) = mpsc::channel();
        *fake.gate.lock() = Some(gate);
        let cache = cache_over(fake, Arc::default());
        let start = Instant::now();
        let opener = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            open.send(()).unwrap();
        });
        assert_eq!(cache.get(var).as_deref(), Some("test-key-2"));
        assert!(start.elapsed() >= Duration::from_millis(200));
        assert!(start.elapsed() < FILL_WAIT);
        opener.join().unwrap();
    }

    fn mode(path: &Path) -> u32 {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn the_env_writer_replaces_only_the_line_of_the_variable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        let before = "# keys for Hush\nGROQ_API_KEY=old-key\n\nexport OPENAI_API_KEY=\"keep\"\nGROQ_API_KEY=later\nGROQ_API_KEY_2=other";
        std::fs::write(&path, before).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        assert!(write_env_line(&path, "GROQ_API_KEY", Some("test-key-1")).unwrap());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "# keys for Hush\nGROQ_API_KEY='test-key-1'\n\nexport OPENAI_API_KEY=\"keep\"\nGROQ_API_KEY_2=other"
        );
        assert_eq!(mode(&path), 0o600);
        assert_eq!(
            from_file(&path, "GROQ_API_KEY").as_deref(),
            Some("test-key-1")
        );
        assert_eq!(from_file(&path, "OPENAI_API_KEY").as_deref(), Some("keep"));
        assert_eq!(from_file(&path, "GROQ_API_KEY_2").as_deref(), Some("other"));

        assert!(write_env_line(&path, "ANTHROPIC_API_KEY", Some("test-key-$2")).unwrap());
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .ends_with("GROQ_API_KEY_2=other\nANTHROPIC_API_KEY='test-key-$2'\n"));
        assert_eq!(
            from_file(&path, "ANTHROPIC_API_KEY").as_deref(),
            Some("test-key-$2"),
            "no expansion"
        );
        assert!(!write_env_line(&path, "ANTHROPIC_API_KEY", Some("test-key-$2")).unwrap());

        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(leftovers, [OsString::from(".env")], "no temporary file");
    }

    #[test]
    fn the_env_writer_makes_a_private_file_and_refuses_quotes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hush").join(".env");
        assert!(write_env_line(&path, "GROQ_API_KEY", Some("test-key-1")).unwrap());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "GROQ_API_KEY='test-key-1'\n"
        );
        assert_eq!(mode(&path), 0o600);
        for bad in ["test'key", "test\nkey"] {
            let err = write_env_line(&path, "GROQ_API_KEY", Some(bad))
                .unwrap_err()
                .to_string();
            assert!(err.contains("GROQ_API_KEY"), "{err}");
            assert!(!err.contains("test"), "the message never shows the key");
        }
        assert_eq!(
            from_file(&path, "GROQ_API_KEY").as_deref(),
            Some("test-key-1")
        );
    }

    #[test]
    fn a_removal_drops_each_line_of_the_variable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        std::fs::write(&path, "A=1\nGROQ_API_KEY=x\n  GROQ_API_KEY = y\nB=2\n").unwrap();
        assert!(write_env_line(&path, "GROQ_API_KEY", None).unwrap());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "A=1\nB=2\n");
        assert!(!write_env_line(&path, "GROQ_API_KEY", None).unwrap());

        let missing = dir.path().join("missing.env");
        assert!(!write_env_line(&missing, "GROQ_API_KEY", None).unwrap());
        assert!(!missing.exists(), "a removal makes no file");
    }

    #[test]
    fn a_key_goes_to_the_keyring_natively_and_to_the_env_file_in_the_sandbox() {
        let dir = tempfile::tempdir().unwrap();
        let env_file = dir.path().join(".env");
        let fake = FakeKeyring::default();
        store_in(Some(&fake), &env_file, "GROQ_API_KEY", " test-key-1 ").unwrap();
        assert_eq!(
            *fake.stored.lock(),
            [("GROQ_API_KEY".to_string(), "test-key-1".to_string())]
        );
        assert!(!env_file.exists(), "natively the .env file stays as it is");

        store_in(None, &env_file, "GROQ_API_KEY", "test-key-2").unwrap();
        assert_eq!(
            from_file(&env_file, "GROQ_API_KEY").as_deref(),
            Some("test-key-2")
        );

        for (var, value) in [("GROQ_API_KEY", "  "), ("not a name", "test-key-3")] {
            assert!(store_in(Some(&fake), &env_file, var, value).is_err());
        }
        assert_eq!(fake.stored.lock().len(), 1);
    }

    #[test]
    fn a_removal_clears_the_keyring_and_the_env_file() {
        let dir = tempfile::tempdir().unwrap();
        let env_file = dir.path().join(".env");
        std::fs::write(
            &env_file,
            "GROQ_API_KEY=test-key-1\nOPENAI_API_KEY=test-key-2\n",
        )
        .unwrap();
        let fake = FakeKeyring::default();
        remove_in(Some(&fake), &env_file, "GROQ_API_KEY").unwrap();
        assert_eq!(*fake.removed.lock(), ["GROQ_API_KEY"]);
        assert_eq!(
            std::fs::read_to_string(&env_file).unwrap(),
            "OPENAI_API_KEY=test-key-2\n"
        );
        remove_in(None, &env_file, "OPENAI_API_KEY").unwrap();
        assert_eq!(std::fs::read_to_string(&env_file).unwrap(), "");
    }

    #[test]
    fn a_debug_line_never_shows_the_key() {
        let given = ApiKey::from("test-key-1");
        assert_eq!(format!("{:?}", given), "ApiKey(..)");
        let var = ApiKey::var("GROQ_API_KEY", Path::new("/x/.env"));
        assert_eq!(format!("{:?}", var), "ApiKey(GROQ_API_KEY)");
        assert!(ApiKey::from("  ").is_blank());
        assert!(!given.is_blank() && !var.is_blank());
    }

    #[tokio::test]
    async fn an_api_key_from_a_variable_reads_the_file_at_each_request() {
        let var = "HUSH_TEST_KEYS_LATER";
        let dir = tempfile::tempdir().unwrap();
        let env_file = dir.path().join(".env");
        let key = ApiKey::var(var, &env_file);
        assert_eq!(key.read().await, None);
        std::fs::write(&env_file, format!("{}=test-key-1\n", var)).unwrap();
        assert_eq!(key.read().await.as_deref(), Some("test-key-1"));
        assert_eq!(ApiKey::from(" ").read().await, None);
    }
}
