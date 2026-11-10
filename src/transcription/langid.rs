//! Limited language detection: the VoxLingua107 language-ID model picks the
//! language of a take among `transcription.detect_languages`.
//!
//! The process holds one model for every transcriber, so an idle unload does
//! not lose it and a download that finishes late still reaches the next
//! take. Each take without the model looks for a valid file on disk first,
//! which costs a `stat` while the file stays the same. When there is none, a
//! download starts in the background: one at a time, and after a failure
//! only when its wait is over. The wait starts at 5 minutes and doubles up to
//! an hour, so a daemon that starts before the network still gets the model.

use crate::transcription::language::Language;
use crate::transcription::models::{ChecksumMismatch, ModelManager};
use crate::transcription::whisper::{cpu_options, cpu_threads};
use crate::Result;
use parking_lot::Mutex;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};
use transcribe_cpp::{LangIdOptions, LangIdSession, LangIdSessionOptions, Model, Role};

const FIRST_WAIT: Duration = Duration::from_secs(5 * 60);
const LONGEST_WAIT: Duration = Duration::from_secs(60 * 60);

/// The VoxLingua107 language-ID model, limited to the languages in
/// `transcription.detect_languages` that it knows.
pub struct LanguageId {
    session: Mutex<LangIdSession>,
    /// The model's label index and the Whisper code of each allowed language
    labels: Vec<(i32, &'static str)>,
    /// The model refuses shorter audio.
    min_samples: usize,
}

impl LanguageId {
    pub fn load(model_path: &Path, languages: &[String]) -> Result<Self> {
        let load_error = |e: transcribe_cpp::Error| {
            anyhow::anyhow!(
                "Failed to load the language-ID model {}: {}",
                model_path.display(),
                e
            )
        };
        crate::transcription::device::init_backends()?;
        let model = Model::load_with(model_path, &cpu_options()).map_err(load_error)?;
        if !model.roles().contains(Role::LangId) {
            return Err(anyhow::anyhow!(
                "{} is not a language-ID model",
                model_path.display()
            ));
        }
        let mut labels: Vec<(i32, &'static str)> = Vec::new();
        let mut unknown = Vec::new();
        for language in languages
            .iter()
            .filter_map(|code| Language::parse(code))
            .filter(|language| !language.is_auto())
        {
            match model.langid_label_index(language.code()) {
                Some(index) if labels.iter().all(|(known, _)| *known != index) => {
                    labels.push((index, language.code()));
                },
                Some(_) => {},
                None => unknown.push(language.code()),
            }
        }
        if !unknown.is_empty() {
            warn!(
                "The language-ID model does not know {}, so limited detection leaves it out",
                unknown.join(", ")
            );
        }
        if labels.is_empty() {
            return Err(anyhow::anyhow!(
                "The language-ID model knows none of {}",
                languages.join(", ")
            ));
        }
        let info = model.langid_info().map_err(load_error)?;
        let min_samples =
            usize::try_from(i64::from(info.min_audio_ms) * i64::from(info.sample_rate) / 1000)
                .unwrap_or(0);
        let session = model
            .langid_session_with(&LangIdSessionOptions {
                n_threads: cpu_threads().min(4),
            })
            .map_err(load_error)?;
        Ok(Self {
            session: Mutex::new(session),
            labels,
            min_samples,
        })
    }

    /// The most likely allowed language in the first 30 seconds of
    /// `samples`, its probability among the allowed languages, and the share
    /// of all probability that the allowed languages hold. `None` when the
    /// speech is too short to tell.
    fn most_likely(&self, samples: &[f32]) -> Result<Option<(&'static str, f32, f32)>> {
        let options = LangIdOptions {
            allowed: Some(
                self.labels
                    .iter()
                    .map(|(_, code)| code.to_string())
                    .collect(),
            ),
        };
        let result = match self.session.lock().run(samples, &options) {
            Ok(result) => result,
            Err(transcribe_cpp::Error::InputTooShort(_)) => return Ok(None),
            Err(e) => return Err(anyhow::anyhow!("{}", e)),
        };
        let Some(best) = result.candidates.first() else {
            return Ok(None);
        };
        Ok(self
            .labels
            .iter()
            .find(|(index, _)| *index == best.index)
            .map(|(_, code)| (*code, best.p, result.allowed_mass)))
    }
}

/// When the next download may start after a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Retry {
    next: Option<Instant>,
    wait: Duration,
}

impl Retry {
    fn new() -> Self {
        Self {
            next: None,
            wait: FIRST_WAIT,
        }
    }

    fn ready(&self, now: Instant) -> bool {
        self.next.is_none_or(|next| now >= next)
    }

    /// Note a failure at `now`, and give the wait before the next try.
    fn failed(&mut self, now: Instant) -> Duration {
        let wait = self.wait;
        self.next = Some(now + wait);
        self.wait = (wait * 2).min(LONGEST_WAIT);
        wait
    }
}

struct Store {
    /// The loaded model and the languages it picks among
    loaded: Option<(Vec<String>, Arc<LanguageId>)>,
    /// Languages that the model failed to load for, so that each take does
    /// not try again
    failed_for: Option<Vec<String>>,
    /// The languages of the last request, for the model that a download
    /// brings
    wanted: Option<Vec<String>>,
    downloading: bool,
    retry: Retry,
    /// A download failed its SHA-256 check, so the published file differs
    /// from the pinned one, and every new try would get the same bytes.
    gave_up: bool,
    /// The language that the last take with limited detection confirmed
    confirmed: Option<&'static str>,
}

static STORE: LazyLock<Mutex<Store>> = LazyLock::new(|| {
    Mutex::new(Store {
        loaded: None,
        failed_for: None,
        wanted: None,
        downloading: false,
        retry: Retry::new(),
        gave_up: false,
        confirmed: None,
    })
});

/// The language-ID model for `languages`: the loaded one, or the valid file
/// in `models_dir`, loaded now. Without a file, a download starts in the
/// background when none runs and the wait after a failure is over. Call it
/// from a Tokio runtime or a thread of its blocking pool; it never waits on
/// the network.
pub fn model(models_dir: &Path, languages: &[String]) -> Option<Arc<LanguageId>> {
    let mut store = STORE.lock();
    if let Some((_, model)) = store
        .loaded
        .as_ref()
        .filter(|(for_languages, _)| for_languages == languages)
    {
        return Some(Arc::clone(model));
    }
    store.wanted = Some(languages.to_vec());
    let manager = ModelManager::new(models_dir).ok()?;
    if let Some(path) = manager.langid_model_path() {
        return load_into(&mut store, &path, languages);
    }
    start_download(&mut store, manager);
    None
}

fn load_into(store: &mut Store, path: &Path, languages: &[String]) -> Option<Arc<LanguageId>> {
    if store.failed_for.as_deref() == Some(languages) {
        return None;
    }
    match LanguageId::load(path, languages) {
        Ok(model) => {
            let model = Arc::new(model);
            store.loaded = Some((languages.to_vec(), Arc::clone(&model)));
            info!("Language identification is ready");
            Some(model)
        },
        Err(e) => {
            warn!(
                "Whisper detects the language within detect_languages, because the \
                 language-ID model did not load: {:#}",
                e
            );
            store.failed_for = Some(languages.to_vec());
            None
        },
    }
}

/// Load the model that a download brought, for the languages of the last
/// request, so that the next take does not wait for the load.
fn load_downloaded(path: PathBuf) {
    let mut store = STORE.lock();
    if let Some(languages) = store.wanted.clone() {
        load_into(&mut store, &path, &languages);
    }
}

/// Whether a failed download should stop all later tries in this process.
fn gives_up(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| cause.downcast_ref::<ChecksumMismatch>().is_some())
}

fn start_download(store: &mut Store, manager: ModelManager) {
    if store.gave_up || store.downloading || !store.retry.ready(Instant::now()) {
        return;
    }
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        debug!("No runtime to download the language-ID model on");
        return;
    };
    store.downloading = true;
    runtime.spawn(async move {
        info!("Downloading the language-ID model that detect_languages needs");
        let result = manager.ensure_langid_downloaded_with(&|_, _| {}).await;
        let downloaded = {
            let mut store = STORE.lock();
            store.downloading = false;
            match result {
                Ok(path) => {
                    store.retry = Retry::new();
                    info!("The language-ID model is downloaded");
                    Some(path)
                },
                Err(e) if gives_up(&e) => {
                    store.gave_up = true;
                    warn!(
                        "The language-ID model did not download ({:#}). Hush does not try \
                         again until it restarts, and Whisper detects the language within \
                         detect_languages",
                        e
                    );
                    None
                },
                Err(e) => {
                    let wait = store.retry.failed(Instant::now());
                    warn!(
                        "The language-ID model did not download ({:#}). Whisper detects the \
                         language within detect_languages, and Hush tries again in {} minutes",
                        e,
                        wait.as_secs() / 60
                    );
                    None
                },
            }
        };
        if let Some(path) = downloaded {
            if let Err(e) = tokio::task::spawn_blocking(move || load_downloaded(path)).await {
                warn!("The language-ID model load stopped: {}", e);
            }
        }
    });
}

/// The audio that language ID hears: the speech, or the whole take when the
/// speech is too short for the model, or the speech with silence after it
/// when both are too short. A one-word take keeps its limited detection.
fn input<'a>(take: &'a [f32], speech: &'a [f32], min_samples: usize) -> Cow<'a, [f32]> {
    if speech.len() >= min_samples {
        return Cow::Borrowed(speech);
    }
    if take.len() >= min_samples {
        return Cow::Borrowed(take);
    }
    let mut padded = speech.to_vec();
    padded.resize(min_samples, 0.0);
    Cow::Owned(padded)
}

/// The most likely of the `allowed` languages in the first 30 seconds of the
/// speech in `take`. When the model is not ready or fails, the answer is
/// `None`, and Whisper detects within the set instead.
pub(crate) fn pick(
    models_dir: &Path,
    take: &[f32],
    speech: &[f32],
    allowed: &[String],
) -> Option<&'static str> {
    let Some(langid) = model(models_dir, allowed) else {
        debug!("The language-ID model is not ready, so Whisper detects the language");
        return None;
    };
    match langid.most_likely(&input(take, speech, langid.min_samples)) {
        Ok(Some((code, probability, allowed_mass))) => {
            info!(
                language = code,
                probability,
                allowed_mass,
                "Language ID picked {} among {}",
                code,
                allowed.join(", ")
            );
            confirm(code);
            Some(code)
        },
        Ok(None) => {
            info!("The language-ID model could not tell the language, so Whisper detects it");
            None
        },
        Err(e) => {
            warn!("Language detection failed, so Whisper detects the language: {e:#}");
            None
        },
    }
}

/// Note the language that language ID or Whisper found within the allowed
/// set for a take.
pub(crate) fn confirm(code: &'static str) {
    STORE.lock().confirmed = Some(code);
}

/// The language for a take that Whisper heard outside `allowed`: the one that
/// the last take confirmed, when it is allowed and the model knows it, else
/// the first allowed language that the model knows. transcribe.cpp gives no
/// score for a Whisper run, so the take cannot run in each language and keep
/// the most likely text.
pub(crate) fn fallback(allowed: &[String], known: &[String]) -> Option<&'static str> {
    fallback_from(STORE.lock().confirmed, allowed, known)
}

fn fallback_from(
    confirmed: Option<&'static str>,
    allowed: &[String],
    known: &[String],
) -> Option<&'static str> {
    confirmed
        .filter(|code| allowed_code(code, allowed).is_some() && knows(known, code))
        .or_else(|| first_allowed(allowed, known))
}

fn knows(known: &[String], code: &str) -> bool {
    known.is_empty() || known.iter().any(|k| k == code)
}

/// `code` as a Whisper language, when it is one of `allowed`.
pub(crate) fn allowed_code(code: &str, allowed: &[String]) -> Option<&'static str> {
    allowed
        .iter()
        .any(|wanted| wanted.trim() == code)
        .then(|| Language::parse(code))
        .flatten()
        .filter(|language| !language.is_auto())
        .map(Language::code)
}

/// The first of `allowed` that the model knows.
fn first_allowed(allowed: &[String], known: &[String]) -> Option<&'static str> {
    allowed
        .iter()
        .filter_map(|code| Language::parse(code))
        .filter(|language| !language.is_auto())
        .map(Language::code)
        .find(|code| knows(known, code))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(list: &[&str]) -> Vec<String> {
        list.iter().map(|code| code.to_string()).collect()
    }

    #[test]
    fn a_language_id_model_that_is_missing_is_an_error() {
        let err = LanguageId::load(Path::new("/nonexistent/lang-id.gguf"), &["en".into()])
            .err()
            .expect("a missing language-ID model must fail");
        assert!(err.to_string().contains("language-ID"), "{err}");
    }

    #[test]
    fn short_speech_still_reaches_language_id() {
        let speech = vec![0.5f32; 4_000];
        let take = vec![0.25f32; 20_000];
        assert_eq!(input(&take, &take, 8_000).len(), 20_000);
        let whole = input(&take, &speech, 8_000);
        assert_eq!(&whole[..], &take[..], "short speech: the whole take");
        let short_take = vec![0.25f32; 6_000];
        let padded = input(&short_take, &speech, 8_000);
        assert_eq!(padded.len(), 8_000, "both short: padded to the minimum");
        assert_eq!(&padded[..4_000], &speech[..]);
        assert!(padded[4_000..].iter().all(|s| *s == 0.0));
    }

    #[test]
    fn a_failed_download_waits_longer_each_time_up_to_an_hour() {
        let start = Instant::now();
        let mut retry = Retry::new();
        assert!(retry.ready(start));
        assert_eq!(retry.failed(start), Duration::from_secs(300));
        assert!(!retry.ready(start + Duration::from_secs(299)));
        assert!(retry.ready(start + Duration::from_secs(300)));
        let waits: Vec<u64> = (0..6).map(|_| retry.failed(start).as_secs() / 60).collect();
        assert_eq!(waits, [10, 20, 40, 60, 60, 60]);
        assert_eq!(Retry::new(), Retry::new(), "a success starts over");
    }

    #[test]
    fn only_a_checksum_mismatch_stops_the_downloads() {
        let mismatch = anyhow::Error::from(ChecksumMismatch {
            name: "voxlingua107-langid".to_string(),
        })
        .context("download");
        assert!(gives_up(&mismatch));
        assert!(!gives_up(&anyhow::anyhow!(
            "dns error: failed to lookup address"
        )));
    }

    #[test]
    fn whisper_detection_stays_within_the_allowed_languages() {
        let allowed = codes(&["de", "fr"]);
        assert_eq!(allowed_code("de", &allowed), Some("de"));
        assert_eq!(allowed_code("nl", &allowed), None, "Dutch is not allowed");
        assert_eq!(allowed_code("auto", &codes(&["auto"])), None);
        assert_eq!(first_allowed(&allowed, &[]), Some("de"));
        let known = codes(&["en", "fr"]);
        assert_eq!(
            first_allowed(&allowed, &known),
            Some("fr"),
            "the first the model knows"
        );
        assert_eq!(first_allowed(&allowed, &codes(&["en"])), None);
    }

    #[test]
    fn a_miss_runs_again_in_the_last_confirmed_language() {
        let allowed = codes(&["en", "de"]);
        assert_eq!(
            fallback_from(Some("de"), &allowed, &[]),
            Some("de"),
            "German heard as Dutch stays German after a German take"
        );
        assert_eq!(
            fallback_from(None, &allowed, &[]),
            Some("en"),
            "else the first"
        );
        assert_eq!(
            fallback_from(Some("fr"), &allowed, &[]),
            Some("en"),
            "a confirmed language outside the set does not count"
        );
        assert_eq!(
            fallback_from(Some("de"), &allowed, &codes(&["en"])),
            Some("en"),
            "nor one the model does not know"
        );
    }
}
