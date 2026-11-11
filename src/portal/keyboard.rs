//! Typing through the RemoteDesktop portal.
//!
//! Each insertion opens a session with the keyboard, sends its keys, and
//! closes the session, because GNOME shows a sharing indicator while a
//! session is open. The session asks for persist mode 2, and the restore
//! token goes to a private file, so the consent dialog shows once. After a
//! refusal, Hush does not ask again until the daemon restarts, so a user who
//! said no sees no dialog at each take.

use super::keysym::KeyStep;
use super::token::TokenStore;
use super::{ConsentRefused, PortalError};
use async_trait::async_trait;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tracing::{debug, info, warn};

/// After the consent dialog closes, the window that had the focus needs a
/// moment to get it back before the first key.
const AFTER_CONSENT: Duration = Duration::from_millis(300);

/// A session start that takes longer than this waited for a person. With
/// no dialog, the portal answers in tens of milliseconds. A token can still
/// bring a dialog: KDE asks again when the token is stale, and GNOME asks
/// when the token is from another session.
const DIALOG_SHOWN_AFTER: Duration = Duration::from_millis(500);

/// Whether a session start of `took` showed the consent dialog.
pub fn dialog_shown(took: Duration) -> bool {
    took >= DIALOG_SHOWN_AFTER
}

/// One RemoteDesktop session for the keyboard.
#[async_trait]
pub trait RemoteDesktopPortal: Send + Sync {
    /// Opens a session with `token`, runs the steps, and closes it. Returns
    /// the token for the next session.
    async fn run(
        &self,
        token: Option<&str>,
        steps: &[KeyStep],
    ) -> Result<Option<String>, PortalError>;
}

/// The consent state and the restore token around the portal.
pub struct Typist<P> {
    portal: P,
    store: TokenStore,
    refused: bool,
}

impl<P: RemoteDesktopPortal> Typist<P> {
    pub fn new(portal: P, store: TokenStore) -> Self {
        Self {
            portal,
            store,
            refused: false,
        }
    }

    pub async fn send(&mut self, steps: &[KeyStep]) -> Result<(), PortalError> {
        if self.refused {
            return Err(PortalError::Refused);
        }
        let token = self.store.load();
        match self.portal.run(token.as_deref(), steps).await {
            Ok(Some(next)) => {
                if token.as_deref() != Some(next.as_str()) {
                    if let Err(e) = self.store.save(&next) {
                        warn!(
                            "Cannot keep the keyboard consent in {}: {}; the desktop asks again",
                            self.store.path().display(),
                            e
                        );
                    }
                }
                Ok(())
            },
            Ok(None) => Ok(()),
            Err(PortalError::Refused) => {
                info!("The desktop refused keyboard access; Hush asks again after a restart");
                self.refused = true;
                if let Err(e) = self.store.clear() {
                    debug!("Cannot remove the old keyboard consent: {}", e);
                }
                Err(PortalError::Refused)
            },
            Err(e) => Err(e),
        }
    }
}

/// The RemoteDesktop portal through ashpd.
pub struct AshpdRemoteDesktop;

#[async_trait]
impl RemoteDesktopPortal for AshpdRemoteDesktop {
    async fn run(
        &self,
        token: Option<&str>,
        steps: &[KeyStep],
    ) -> Result<Option<String>, PortalError> {
        use ashpd::desktop::remote_desktop::{
            DeviceType, KeyState, RemoteDesktop, SelectDevicesOptions, StartOptions,
        };
        use ashpd::desktop::PersistMode;
        let error = |e| PortalError::from_ashpd("RemoteDesktop", e);
        let proxy = RemoteDesktop::new().await.map_err(error)?;
        let session = proxy
            .create_session(Default::default())
            .await
            .map_err(error)?;
        let result = async {
            let asked = std::time::Instant::now();
            proxy
                .select_devices(
                    &session,
                    SelectDevicesOptions::default()
                        .set_devices(ashpd::enumflags2::BitFlags::from(DeviceType::Keyboard))
                        .set_persist_mode(PersistMode::ExplicitlyRevoked)
                        .set_restore_token(token),
                )
                .await
                .map_err(error)?
                .response()
                .map_err(error)?;
            let started = proxy
                .start(&session, None, StartOptions::default())
                .await
                .map_err(error)?
                .response()
                .map_err(error)?;
            if !started.devices().contains(DeviceType::Keyboard) {
                return Err(PortalError::Refused);
            }
            if dialog_shown(asked.elapsed()) {
                tokio::time::sleep(AFTER_CONSENT).await;
            }
            for &step in steps {
                let state = if matches!(step, KeyStep::SymPress(_) | KeyStep::CodePress(_)) {
                    KeyState::Pressed
                } else {
                    KeyState::Released
                };
                match step {
                    KeyStep::SymPress(sym) | KeyStep::SymRelease(sym) => {
                        let sym = i32::try_from(sym)
                            .map_err(|_| PortalError::Failed(format!("keysym {sym:#x}")))?;
                        proxy
                            .notify_keyboard_keysym(&session, sym, state, Default::default())
                            .await
                            .map_err(error)?;
                    },
                    KeyStep::CodePress(code) | KeyStep::CodeRelease(code) => proxy
                        .notify_keyboard_keycode(
                            &session,
                            i32::from(code),
                            state,
                            Default::default(),
                        )
                        .await
                        .map_err(error)?,
                    KeyStep::Pause(delay) => tokio::time::sleep(delay).await,
                }
            }
            Ok(started.restore_token().map(str::to_string))
        }
        .await;
        if let Err(e) = session.close().await {
            debug!("Cannot close the RemoteDesktop session: {}", e);
        }
        result
    }
}

type Reply = mpsc::Sender<Result<(), PortalError>>;

/// A blocking front for the insertion thread. The portal runs on a thread
/// with a runtime of its own, because the caller can run inside the
/// daemon's runtime, which cannot block on another.
pub struct PortalKeyboard {
    requests: mpsc::Sender<(Vec<KeyStep>, Reply)>,
}

impl PortalKeyboard {
    pub fn new() -> anyhow::Result<Self> {
        Self::with_portal(
            AshpdRemoteDesktop,
            TokenStore::new(TokenStore::default_path()),
        )
    }

    pub fn with_portal<P: RemoteDesktopPortal + 'static>(
        portal: P,
        store: TokenStore,
    ) -> anyhow::Result<Self> {
        let (requests, inbox) = mpsc::channel::<(Vec<KeyStep>, Reply)>();
        thread::Builder::new()
            .name("hush-remote-desktop".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(e) => {
                        warn!("Cannot start the RemoteDesktop runtime: {}", e);
                        return;
                    },
                };
                let mut typist = Typist::new(portal, store);
                while let Ok((steps, reply)) = inbox.recv() {
                    let sent = runtime.block_on(typist.send(&steps));
                    let _ = reply.send(sent);
                }
            })
            .map_err(|e| anyhow::anyhow!("cannot start the RemoteDesktop thread: {}", e))?;
        Ok(Self { requests })
    }

    /// Sends the steps in one session. A refusal comes back as
    /// [`ConsentRefused`], so the daemon can say what to do.
    pub fn send(&self, steps: Vec<KeyStep>) -> anyhow::Result<()> {
        let (reply, answer) = mpsc::channel();
        self.requests
            .send((steps, reply))
            .map_err(|_| anyhow::anyhow!("the RemoteDesktop thread stopped"))?;
        match answer.recv() {
            Ok(Ok(())) => Ok(()),
            Ok(Err(PortalError::Refused)) => Err(ConsentRefused.into()),
            Ok(Err(e)) => Err(anyhow::anyhow!("{}", e)),
            Err(_) => Err(anyhow::anyhow!("the RemoteDesktop thread stopped")),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use parking_lot::Mutex;
    use std::collections::VecDeque;
    use std::sync::Arc;

    /// The token a session brought and the steps it sent.
    type Session = (Option<String>, Vec<KeyStep>);
    type Answer = Result<Option<String>, PortalError>;

    /// A portal that records each session and answers from a script.
    #[derive(Clone, Default)]
    pub(crate) struct MockPortal {
        pub sessions: Arc<Mutex<Vec<Session>>>,
        pub answers: Arc<Mutex<VecDeque<Answer>>>,
    }

    impl MockPortal {
        pub fn answering(answers: Vec<Answer>) -> Self {
            let mock = Self::default();
            mock.answers.lock().extend(answers);
            mock
        }
    }

    #[async_trait]
    impl RemoteDesktopPortal for MockPortal {
        async fn run(
            &self,
            token: Option<&str>,
            steps: &[KeyStep],
        ) -> Result<Option<String>, PortalError> {
            self.sessions
                .lock()
                .push((token.map(str::to_string), steps.to_vec()));
            self.answers.lock().pop_front().unwrap_or(Ok(None))
        }
    }

    fn store() -> (tempfile::TempDir, TokenStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = TokenStore::new(dir.path().join("token"));
        (dir, store)
    }

    const A: [KeyStep; 2] = [KeyStep::SymPress(0x61), KeyStep::SymRelease(0x61)];

    #[tokio::test]
    async fn the_first_session_asks_and_the_next_brings_the_token() {
        let (_dir, store) = store();
        let portal = MockPortal::answering(vec![Ok(Some("t1".into())), Ok(Some("t2".into()))]);
        let mut typist = Typist::new(portal.clone(), store.clone());
        typist.send(&A).await.unwrap();
        typist.send(&A).await.unwrap();
        let sessions = portal.sessions.lock().clone();
        assert_eq!(
            sessions[0],
            (None, A.to_vec()),
            "no token: the dialog shows"
        );
        assert_eq!(sessions[1], (Some("t1".into()), A.to_vec()));
        assert_eq!(
            store.load().as_deref(),
            Some("t2"),
            "the newest token is kept"
        );
    }

    #[tokio::test]
    async fn a_refusal_stops_the_dialogs_until_a_restart() {
        let (_dir, store) = store();
        store.save("revoked").unwrap();
        let portal = MockPortal::answering(vec![Err(PortalError::Refused)]);
        let mut typist = Typist::new(portal.clone(), store.clone());
        assert_eq!(typist.send(&A).await, Err(PortalError::Refused));
        assert_eq!(typist.send(&A).await, Err(PortalError::Refused));
        assert_eq!(portal.sessions.lock().len(), 1, "no second dialog");
        assert_eq!(store.load(), None, "the refused token goes");

        let restarted = MockPortal::answering(vec![Ok(Some("t".into()))]);
        let mut typist = Typist::new(restarted.clone(), store.clone());
        typist.send(&A).await.unwrap();
        assert_eq!(restarted.sessions.lock().len(), 1, "a restart asks again");
    }

    #[tokio::test]
    async fn a_missing_portal_is_an_error_and_keeps_the_token() {
        let (_dir, store) = store();
        store.save("t").unwrap();
        let portal = MockPortal::answering(vec![
            Err(PortalError::Unavailable("RemoteDesktop".into())),
            Ok(None),
        ]);
        let mut typist = Typist::new(portal.clone(), store.clone());
        assert!(matches!(
            typist.send(&A).await,
            Err(PortalError::Unavailable(_))
        ));
        assert_eq!(store.load().as_deref(), Some("t"));
        typist.send(&A).await.unwrap();
        assert_eq!(
            store.load().as_deref(),
            Some("t"),
            "no new token keeps the old one"
        );
    }

    #[test]
    fn only_a_slow_start_waits_for_the_focus() {
        assert!(!dialog_shown(Duration::from_millis(40)), "a kept token");
        assert!(!dialog_shown(Duration::from_millis(499)));
        assert!(dialog_shown(Duration::from_millis(500)));
        assert!(
            dialog_shown(Duration::from_secs(4)),
            "a person read the dialog"
        );
    }

    #[test]
    fn the_blocking_front_reports_a_refusal_as_its_own_error() {
        let (_dir, store) = store();
        let portal = MockPortal::answering(vec![Ok(Some("t".into())), Err(PortalError::Refused)]);
        let keyboard = PortalKeyboard::with_portal(portal.clone(), store).unwrap();
        keyboard.send(A.to_vec()).unwrap();
        let error = keyboard.send(A.to_vec()).unwrap_err();
        assert!(error.downcast_ref::<ConsentRefused>().is_some(), "{error}");
        assert_eq!(portal.sessions.lock().len(), 2);
    }
}
