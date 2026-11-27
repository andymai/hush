//! The Secret Service on the session bus (GNOME Keyring, KWallet), through
//! oo7. Each item of Hush has the attributes `application = "hush"` and
//! `variable = <VAR>`.
//!
//! Each call runs on a thread of its own with its own runtime, because
//! callers reach here from the daemon's async context, from the settings
//! window, and from plain threads.
//!
//! Before each call, Hush asks the bus whether a Secret Service runs. A
//! call to `org.freedesktop.secrets` itself would start a keyring daemon
//! through D-Bus activation, and with no keyring daemon Hush acts as if
//! the keyring held nothing.

use super::keys::{Found, SecretStore, Unlocked, APPLICATION, VARIABLE};
use anyhow::{anyhow, bail, Context, Result};
use oo7::dbus::{Item, Service};
use oo7::zbus;
use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;
use tracing::debug;

const SECRETS: &str = "org.freedesktop.secrets";
const SERVICE_PATH: &str = "/org/freedesktop/secrets";
const SERVICE_INTERFACE: &str = "org.freedesktop.Secret.Service";
/// How long one call waits for its answer. An unlock prompt waits for a
/// signal, so this limit does not cut it short.
const CALL_TIMEOUT: Duration = Duration::from_secs(5);

pub struct SecretService;

impl SecretStore for SecretService {
    fn find(&self, var: &str) -> Result<Found> {
        on_own_runtime(find(var))
    }

    fn read_unlocked(&self) -> Result<Option<Unlocked>> {
        on_own_runtime(read_unlocked())
    }

    fn store(&self, var: &str, value: &str) -> Result<()> {
        on_own_runtime(store(var, value))
    }

    fn remove(&self, var: &str) -> Result<bool> {
        on_own_runtime(remove(var))
    }
}

fn on_own_runtime<T: Send>(work: impl Future<Output = Result<T>> + Send) -> Result<T> {
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .context("Failed to start a runtime for the Secret Service")?
                    .block_on(work)
            })
            .join()
            .map_err(|_| anyhow!("The Secret Service call stopped"))?
    })
}

fn attributes(var: Option<&str>) -> HashMap<&str, &str> {
    let mut attributes = HashMap::from([APPLICATION]);
    if let Some(var) = var {
        attributes.insert(VARIABLE, var);
    }
    attributes
}

/// A connection to the session bus, when a Secret Service owns its name.
async fn connect() -> Result<Option<zbus::Connection>> {
    let connection = zbus::connection::Builder::session()
        .method_timeout(CALL_TIMEOUT)
        .build()
        .await
        .context("No session bus")?;
    let owned: bool = connection
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "NameHasOwner",
            &(SECRETS,),
        )
        .await?
        .body()
        .deserialize()?;
    Ok(owned.then_some(connection))
}

/// The unlocked and the locked items with `attributes`. The service's own
/// search lists the locked items too, which a collection's search may not.
async fn search(
    connection: &zbus::Connection,
    attributes: &HashMap<&str, &str>,
) -> Result<(Vec<zbus::OwnedObjectPath>, Vec<zbus::OwnedObjectPath>)> {
    Ok(connection
        .call_method(
            Some(SECRETS),
            SERVICE_PATH,
            Some(SERVICE_INTERFACE),
            "SearchItems",
            &(attributes,),
        )
        .await?
        .body()
        .deserialize()?)
}

async fn find(var: &str) -> Result<Found> {
    let Some(connection) = connect().await? else {
        return Ok(Found::Absent);
    };
    let (unlocked, locked) = search(&connection, &attributes(Some(var))).await?;
    Ok(if !unlocked.is_empty() {
        Found::Unlocked
    } else if !locked.is_empty() {
        Found::Locked
    } else {
        Found::Absent
    })
}

async fn read_unlocked() -> Result<Option<Unlocked>> {
    let Some(connection) = connect().await? else {
        return Ok(None);
    };
    let wanted = attributes(None);
    let (unlocked, locked) = search(&connection, &wanted).await?;
    let mut values = HashMap::new();
    if !unlocked.is_empty() {
        let service = Service::new_with_connection(&connection).await?;
        for collection in service.collections().await? {
            if collection.is_locked().await? {
                continue;
            }
            for item in collection.search_items(&wanted).await? {
                match read_item(&item).await {
                    Ok(Some((var, value))) => {
                        values.entry(var).or_insert(value);
                    },
                    Ok(None) => {},
                    Err(e) => debug!("Skipped an item of the keyring: {:#}", e),
                }
            }
        }
    }
    Ok(Some(Unlocked {
        values,
        locked: !locked.is_empty(),
    }))
}

/// The variable and the key of an unlocked item that holds text.
async fn read_item(item: &Item) -> Result<Option<(String, String)>> {
    if item.is_locked().await? {
        return Ok(None);
    }
    let Some(var) = item.attributes().await?.remove(VARIABLE) else {
        return Ok(None);
    };
    let secret = item.secret().await?;
    Ok(std::str::from_utf8(&secret)
        .ok()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| (var, value.to_string())))
}

async fn store(var: &str, value: &str) -> Result<()> {
    let connection = connect().await?.context(
        "No Secret Service runs, such as GNOME Keyring or KWallet. Put the key in \
         ~/.config/hush/.env",
    )?;
    let service = Service::new_with_connection(&connection).await?;
    let collection = service.default_collection().await?;
    if collection.is_locked().await? {
        collection.unlock(None).await?;
        if collection.is_locked().await? {
            bail!("The keyring stays locked");
        }
    }
    collection
        .create_item(
            &format!("Hush: {}", var),
            &attributes(Some(var)),
            value,
            true,
            None,
        )
        .await?;
    Ok(())
}

async fn remove(var: &str) -> Result<bool> {
    let Some(connection) = connect().await? else {
        return Ok(false);
    };
    let wanted = attributes(Some(var));
    let (unlocked, locked) = search(&connection, &wanted).await?;
    if unlocked.is_empty() && locked.is_empty() {
        return Ok(false);
    }
    let service = Service::new_with_connection(&connection).await?;
    let mut removed = false;
    for collection in service.collections().await? {
        let inside = format!("{}/", collection.path().as_str());
        let holds_locked = locked.iter().any(|path| path.as_str().starts_with(&inside));
        if holds_locked && collection.is_locked().await? {
            collection.unlock(None).await?;
        }
        for item in collection.search_items(&wanted).await? {
            if item.is_locked().await? {
                item.unlock(None).await?;
            }
            item.delete(None).await?;
            removed = true;
        }
    }
    Ok(removed)
}

/// These tests talk to a real Secret Service, so they run only on a private
/// session bus with a throwaway home, never on the user's own bus:
///
/// ```text
/// export HOME=$(mktemp -d)
/// HUSH_TEST_KEYRING=running /usr/bin/dbus-run-session -- sh -c \
///   'echo -n test | gnome-keyring-daemon --unlock --components=secrets >/dev/null &&
///    cargo nextest run --lib --run-ignored only -E "test(live_keyring)"'
/// HUSH_TEST_KEYRING=absent /usr/bin/dbus-run-session -- \
///   cargo nextest run --lib --run-ignored only -E "test(live_keyring)"
/// ```
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::keys::KeyCache;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    const VAR: &str = "HUSH_TEST_LIVE_KEY";

    /// Whether this run is for `mode`. A run for it must be on a private
    /// bus: dbus-run-session puts its socket under /tmp, and the user's own
    /// bus is under /run/user.
    fn live(mode: &str) -> bool {
        if std::env::var("HUSH_TEST_KEYRING").ok().as_deref() != Some(mode) {
            eprintln!("Skipped: HUSH_TEST_KEYRING is not {}", mode);
            return false;
        }
        let address = std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap_or_default();
        assert!(
            !address.is_empty() && !address.contains("/run/user/"),
            "not a private session bus: {address}"
        );
        true
    }

    fn bus_call<T>(method: &'static str, body: &'static str) -> T
    where
        T: serde::de::DeserializeOwned + zbus::wire::Type + Send,
    {
        on_own_runtime(async move {
            let connection = zbus::Connection::session().await?;
            let reply = if body.is_empty() {
                connection
                    .call_method(
                        Some("org.freedesktop.DBus"),
                        "/org/freedesktop/DBus",
                        Some("org.freedesktop.DBus"),
                        method,
                        &(),
                    )
                    .await?
            } else {
                connection
                    .call_method(
                        Some("org.freedesktop.DBus"),
                        "/org/freedesktop/DBus",
                        Some("org.freedesktop.DBus"),
                        method,
                        &(body,),
                    )
                    .await?
            };
            Ok(reply.body().deserialize::<T>()?)
        })
        .unwrap()
    }

    fn owned() -> bool {
        bus_call("NameHasOwner", SECRETS)
    }

    fn counting_cache(notices: &Arc<AtomicUsize>) -> Arc<KeyCache> {
        let notices = Arc::clone(notices);
        Arc::new(KeyCache::new(
            Arc::new(SecretService),
            Box::new(move || {
                notices.fetch_add(1, Ordering::SeqCst);
            }),
        ))
    }

    /// The labels and values of the items of [`VAR`] in the default
    /// collection.
    fn items() -> Vec<(String, String)> {
        on_own_runtime(async {
            let connection = zbus::Connection::session().await?;
            let service = Service::new_with_connection(&connection).await?;
            let collection = service.default_collection().await?;
            let mut found = Vec::new();
            for item in collection.search_items(&attributes(Some(VAR))).await? {
                let value = String::from_utf8(item.secret().await?.to_vec())?;
                found.push((item.label().await?, value));
            }
            Ok(found)
        })
        .unwrap()
    }

    fn lock_default_collection() {
        on_own_runtime(async {
            let connection = zbus::Connection::session().await?;
            let service = Service::new_with_connection(&connection).await?;
            service.default_collection().await?.lock(None).await?;
            Ok(())
        })
        .unwrap()
    }

    #[test]
    #[ignore = "needs a private session bus with gnome-keyring"]
    fn live_keyring_stores_reads_and_keeps_locked_items_closed() {
        if !live("running") {
            return;
        }
        assert!(owned(), "gnome-keyring runs on the private bus");
        let keyring = SecretService;
        assert_eq!(keyring.find(VAR).unwrap(), Found::Absent);

        keyring.store(VAR, "test-key-live-1").unwrap();
        keyring.store(VAR, "test-key-live-2").unwrap();
        assert_eq!(
            items(),
            [(format!("Hush: {}", VAR), "test-key-live-2".to_string())],
            "one item, replaced"
        );
        assert_eq!(keyring.find(VAR).unwrap(), Found::Unlocked);

        let notices = Arc::new(AtomicUsize::new(0));
        let daemon = counting_cache(&notices);
        daemon.fill();
        assert_eq!(daemon.get(VAR).as_deref(), Some("test-key-live-2"));
        assert_eq!(notices.load(Ordering::SeqCst), 0);

        assert!(keyring.remove(VAR).unwrap());
        assert_eq!(keyring.find(VAR).unwrap(), Found::Absent);
        assert!(!keyring.remove(VAR).unwrap());

        keyring.store(VAR, "test-key-live-3").unwrap();
        lock_default_collection();
        assert_eq!(
            keyring.find(VAR).unwrap(),
            Found::Locked,
            "a search finds the locked item and unlocks nothing"
        );
        let unlocked = keyring
            .read_unlocked()
            .unwrap()
            .expect("a Secret Service runs");
        assert!(unlocked.values.is_empty() && unlocked.locked);

        let daemon = counting_cache(&notices);
        assert_eq!(daemon.get(VAR), None, "no key from the locked keyring");
        assert_eq!(daemon.get(VAR), None);
        assert_eq!(notices.load(Ordering::SeqCst), 1, "one notice");
        assert_eq!(keyring.find(VAR).unwrap(), Found::Locked, "still locked");
    }

    #[test]
    #[ignore = "needs a private session bus that can start gnome-keyring"]
    fn live_keyring_is_never_started_by_hush() {
        if !live("absent") {
            return;
        }
        let activatable: Vec<String> = bus_call("ListActivatableNames", "");
        assert!(
            activatable.iter().any(|name| name == SECRETS),
            "the bus could start a keyring, so the test shows that Hush does not"
        );
        assert!(!owned());

        let keyring = SecretService;
        assert_eq!(keyring.find(VAR).unwrap(), Found::Absent);
        assert_eq!(keyring.read_unlocked().unwrap(), None, "no Secret Service");
        assert!(!keyring.remove(VAR).unwrap());
        let err = keyring.store(VAR, "test-key-live-4").unwrap_err();
        assert!(format!("{err:#}").contains("No Secret Service"), "{err:#}");
        let notices = Arc::new(AtomicUsize::new(0));
        assert_eq!(counting_cache(&notices).get(VAR), None);
        assert_eq!(notices.load(Ordering::SeqCst), 0);

        std::thread::sleep(Duration::from_millis(500));
        assert!(!owned(), "no keyring daemon started");
    }
}
