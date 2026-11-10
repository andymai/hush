//! A hands-free take in parts: [`PartCutter`] cuts the take while it
//! records, and each part goes to a background task that transcribes the
//! parts in order on the model of the take. A part waits for the model when
//! it loads, as a whole take does. The prompt of each part carries the end
//! of the text before it. At the stop only the last part is left, and the
//! texts join with single spaces.
//!
//! Each part stays on disk as FLAC until Hush delivers the joined text. A
//! part that fails moves to the recovery folder for `hush recover`.

use crate::audio::parts::{pad, PartCutter};
use crate::audio::{store, TakeLevel};
use crate::text::WindowInfo;
use crate::text_processing::prompt;
use crate::transcription::{SpeechToText, WHISPER_SAMPLE_RATE};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// What transcribes a part.
#[async_trait]
pub trait PartModel: Send + Sync + 'static {
    fn prompt_fits(&self, prompt: &str) -> bool;
    async fn transcribe_part(&self, audio: &[f32], prompt: Option<&str>) -> Result<String>;
}

#[async_trait]
impl PartModel for SpeechToText {
    fn prompt_fits(&self, prompt: &str) -> bool {
        SpeechToText::prompt_fits(self, prompt)
    }

    async fn transcribe_part(&self, audio: &[f32], prompt: Option<&str>) -> Result<String> {
        Ok(self
            .transcribe_with_prompt(audio, WHISPER_SAMPLE_RATE, prompt)
            .await?
            .text)
    }
}

/// The window and the learned terms that open the prompt of each part,
/// when `transcription.context_prompt` is on.
#[derive(Debug, Clone, Default)]
pub struct PromptContext {
    pub window: Option<WindowInfo>,
    pub terms: Vec<String>,
}

struct Part {
    index: usize,
    audio: Vec<f32>,
    file: Option<PathBuf>,
}

/// The result of one part.
#[derive(Debug)]
pub struct PartOutcome {
    pub index: usize,
    pub file: Option<PathBuf>,
    pub text: Result<String>,
}

/// The task that transcribes the parts in order.
struct PartWorker<M> {
    jobs: Option<mpsc::UnboundedSender<Part>>,
    model: Option<watch::Sender<Option<Arc<M>>>>,
    provided: bool,
    task: Option<JoinHandle<Vec<PartOutcome>>>,
}

impl<M: PartModel> PartWorker<M> {
    fn spawn(context: Option<PromptContext>) -> Self {
        let (jobs, jobs_rx) = mpsc::unbounded_channel();
        let (model, model_rx) = watch::channel(None);
        Self {
            jobs: Some(jobs),
            model: Some(model),
            provided: false,
            task: Some(tokio::spawn(work(jobs_rx, model_rx, context))),
        }
    }

    fn provide(&mut self, model: &Arc<M>) {
        if let Some(sender) = self.model.as_ref().filter(|_| !self.provided) {
            sender.send_replace(Some(Arc::clone(model)));
            self.provided = true;
        }
    }

    fn send(&self, part: Part) {
        if let Some(jobs) = &self.jobs {
            if jobs.send(part).is_err() {
                warn!("The part task stopped before the take ended");
            }
        }
    }

    /// Wait for every part sent. Parts still without a model fail.
    async fn finish(mut self) -> Vec<PartOutcome> {
        self.jobs = None;
        self.model = None;
        match self.task.take() {
            Some(task) => task.await.unwrap_or_else(|e| {
                warn!("The part task failed: {}", e);
                Vec::new()
            }),
            None => Vec::new(),
        }
    }
}

impl<M> Drop for PartWorker<M> {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

async fn work<M: PartModel>(
    mut jobs: mpsc::UnboundedReceiver<Part>,
    mut model: watch::Receiver<Option<Arc<M>>>,
    context: Option<PromptContext>,
) -> Vec<PartOutcome> {
    let mut outcomes = Vec::new();
    let mut ready: Option<(Arc<M>, Option<String>)> = None;
    let mut carried = String::new();
    while let Some(part) = jobs.recv().await {
        if TakeLevel::of(&part.audio) != TakeLevel::Sound {
            debug!(part = part.index, "A part holds no speech");
            outcomes.push(PartOutcome {
                index: part.index,
                file: part.file,
                text: Ok(String::new()),
            });
            continue;
        }
        if ready.is_none() {
            let loaded = model
                .wait_for(Option::is_some)
                .await
                .ok()
                .and_then(|loaded| loaded.as_ref().map(Arc::clone));
            ready = loaded.map(|loaded| {
                let base = context.as_ref().and_then(|context| {
                    prompt::whisper_prompt(context.window.as_ref(), &context.terms, |p| {
                        loaded.prompt_fits(p)
                    })
                });
                (loaded, base)
            });
        }
        let text = match &ready {
            None => Err(anyhow!("The model did not load")),
            Some((loaded, base)) => {
                let prompt =
                    prompt::with_previous(base.as_deref(), &carried, |p| loaded.prompt_fits(p));
                loaded
                    .transcribe_part(&pad(&part.audio), prompt.as_deref())
                    .await
            },
        };
        match &text {
            Ok(text) => {
                debug!(
                    part = part.index,
                    chars = text.chars().count(),
                    "Transcribed a part"
                );
                carried = join(&[std::mem::take(&mut carried), text.clone()]);
            },
            Err(e) => warn!(part = part.index, "A part did not transcribe: {:#}", e),
        }
        outcomes.push(PartOutcome {
            index: part.index,
            file: part.file,
            text,
        });
    }
    outcomes
}

/// The parts of one hands-free take. Make it on a Tokio runtime.
pub struct TakeParts<M> {
    cutter: PartCutter,
    read: usize,
    dir: PathBuf,
    stem: String,
    sent: Vec<(usize, Option<PathBuf>)>,
    worker: PartWorker<M>,
}

impl<M: PartModel> TakeParts<M> {
    /// Parts go to `dir` as `<stem>-partNN.flac`.
    pub fn new(dir: PathBuf, stem: String, context: Option<PromptContext>) -> Self {
        Self {
            cutter: PartCutter::new(),
            read: 0,
            dir,
            stem,
            sent: Vec::new(),
            worker: PartWorker::spawn(context),
        }
    }

    /// The samples of the take that [`TakeParts::feed`] got.
    pub fn read(&self) -> usize {
        self.read
    }

    /// Whether a part ended. Without one, the take is one pass.
    pub fn has_parts(&self) -> bool {
        !self.sent.is_empty()
    }

    pub fn needs_model(&self) -> bool {
        !self.worker.provided
    }

    /// Let the parts transcribe on `model`.
    pub fn provide(&mut self, model: &Arc<M>) {
        self.worker.provide(model);
    }

    /// The next samples of the take. Each part that ends is saved and goes
    /// to the background task.
    pub fn feed(&mut self, samples: &[f32]) {
        self.read += samples.len();
        for part in self.cutter.feed(samples) {
            self.send(part);
        }
    }

    fn send(&mut self, audio: Vec<f32>) {
        let index = self.sent.len() + 1;
        let name = format!("{}-part{:02}", self.stem, index);
        let file = match store::save_as(&self.dir, &name, &audio) {
            Ok(path) => Some(path),
            Err(e) => {
                warn!("Could not keep part {} of the take: {:#}", index, e);
                None
            },
        };
        self.sent.push((index, file.clone()));
        self.worker.send(Part { index, audio, file });
    }

    /// Send the rest of `take` as the last part and wait for every part.
    /// `model` is `None` when it did not load, and then every part fails.
    pub async fn finish(mut self, take: &[f32], model: Option<&Arc<M>>) -> Vec<PartOutcome> {
        if let Some(model) = model {
            self.provide(model);
        }
        let rest = take
            .get(self.cutter.emitted()..)
            .map(<[f32]>::to_vec)
            .unwrap_or_default();
        if !rest.is_empty() {
            self.send(rest);
        }
        let sent = std::mem::take(&mut self.sent);
        let mut outcomes = self.worker.finish().await;
        for (index, file) in sent {
            if !outcomes.iter().any(|outcome| outcome.index == index) {
                outcomes.push(PartOutcome {
                    index,
                    file,
                    text: Err(anyhow!("The part task stopped")),
                });
            }
        }
        outcomes
    }

    /// Stop the task and delete the parts: the take ends without text.
    pub fn discard(self) {
        let files: Vec<PathBuf> = self.sent.iter().filter_map(|(_, f)| f.clone()).collect();
        drop(PartFiles(files));
    }

    /// Stop the task and move the parts to `recovery`.
    pub fn keep(self, recovery: &Path) {
        for file in self.sent.iter().filter_map(|(_, file)| file.as_deref()) {
            if let Err(e) = store::move_to_recovery(file, recovery) {
                warn!("{:#}", e);
            }
        }
    }
}

/// The texts in order, trimmed, without empty ones, one space apart.
pub fn join(texts: &[String]) -> String {
    texts
        .iter()
        .map(|text| text.trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The part files of a take whose text Hush delivers. Dropping the value
/// deletes them, so hold it until the text is inserted.
#[derive(Debug, Default)]
pub struct PartFiles(Vec<PathBuf>);

impl PartFiles {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Drop for PartFiles {
    fn drop(&mut self) {
        for file in &self.0 {
            match std::fs::remove_file(file) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    warn!("Could not delete {}: {}", file.display(), e)
                },
                _ => {},
            }
        }
    }
}

/// The parts of a take, joined.
#[derive(Debug)]
pub struct Settled {
    pub text: String,
    /// The parts that transcribed
    pub files: PartFiles,
    /// The parts that did not transcribe; their files are in the recovery
    /// folder
    pub failed: usize,
}

/// Join the texts in part order, and move the file of each failed part to
/// `recovery`.
pub fn settle(mut outcomes: Vec<PartOutcome>, recovery: &Path) -> Settled {
    info!("A hands-free take ends with {} parts", outcomes.len());
    outcomes.sort_by_key(|outcome| outcome.index);
    let mut texts = Vec::new();
    let mut files = Vec::new();
    let mut failed = 0;
    for outcome in outcomes {
        match outcome.text {
            Ok(text) => {
                texts.push(text);
                files.extend(outcome.file);
            },
            Err(_) => {
                failed += 1;
                let Some(file) = outcome.file else {
                    continue;
                };
                match store::move_to_recovery(&file, recovery) {
                    Ok(kept) => info!(
                        "Kept part {} for `hush recover`: {}",
                        outcome.index,
                        kept.display()
                    ),
                    Err(e) => warn!("{:#}", e),
                }
            },
        }
    }
    Settled {
        text: join(&texts),
        files: PartFiles(files),
        failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    const RATE: usize = WHISPER_SAMPLE_RATE as usize;

    fn speech(secs: f64) -> Vec<f32> {
        (0..(secs * RATE as f64) as usize)
            .map(|i| (i as f32 * 0.07).sin() * 0.3)
            .collect()
    }

    fn quiet(secs: f64) -> Vec<f32> {
        (0..(secs * RATE as f64) as usize)
            .map(|i| if i % 2 == 0 { 0.001 } else { -0.001 })
            .collect()
    }

    /// Three sentences with a pause after each of the first two.
    fn three_sentences() -> Vec<f32> {
        let mut take = Vec::new();
        for _ in 0..2 {
            take.extend(speech(4.0));
            take.extend(quiet(1.2));
        }
        take.extend(speech(2.0));
        take
    }

    /// Answers each call with the next text; "FAIL" is an error.
    struct Fake {
        answers: Mutex<Vec<&'static str>>,
        prompts: Mutex<Vec<Option<String>>>,
    }

    impl Fake {
        fn new(answers: &[&'static str]) -> Arc<Self> {
            let mut answers = answers.to_vec();
            answers.reverse();
            Arc::new(Self {
                answers: Mutex::new(answers),
                prompts: Mutex::new(Vec::new()),
            })
        }

        fn prompts(&self) -> Vec<Option<String>> {
            self.prompts.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl PartModel for Fake {
        fn prompt_fits(&self, prompt: &str) -> bool {
            prompt.chars().count() <= 200
        }

        async fn transcribe_part(&self, audio: &[f32], prompt: Option<&str>) -> Result<String> {
            assert!(
                audio[..3_200].iter().all(|s| *s == 0.0),
                "the part is padded"
            );
            self.prompts
                .lock()
                .unwrap()
                .push(prompt.map(str::to_string));
            match self.answers.lock().unwrap().pop() {
                Some("FAIL") => Err(anyhow!("the GPU stopped")),
                Some(text) => Ok(text.to_string()),
                None => Ok(String::new()),
            }
        }
    }

    fn files_in(dir: &Path) -> Vec<PathBuf> {
        store::list(dir)
            .unwrap()
            .into_iter()
            .map(|kept| kept.path)
            .collect()
    }

    async fn settle_soon() {
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test]
    async fn parts_wait_for_the_model_then_join_in_order_with_the_text_carried() {
        let dir = tempfile::tempdir().unwrap();
        let parts_dir = dir.path().join("parts");
        let recovery = dir.path().join("recover");
        let model = Fake::new(&["First part.", " Second part. ", "Third part."]);
        let context = PromptContext {
            window: None,
            terms: vec!["Kubernetes".to_string()],
        };
        let mut parts = TakeParts::<Fake>::new(parts_dir.clone(), "take".into(), Some(context));
        let take = three_sentences();
        for piece in take.chunks(341) {
            parts.feed(piece);
        }
        assert!(parts.has_parts());
        assert_eq!(parts.read(), take.len());
        settle_soon().await;
        assert!(
            model.prompts().is_empty(),
            "no model yet, so the parts wait"
        );
        assert_eq!(files_in(&parts_dir).len(), 2, "each part is on disk");

        assert!(parts.needs_model());
        parts.provide(&model);
        assert!(!parts.needs_model());
        let outcomes = parts.finish(&take, Some(&model)).await;
        assert_eq!(outcomes.len(), 3);
        assert_eq!(
            model.prompts(),
            [
                Some("Kubernetes.".to_string()),
                Some("Kubernetes. First part.".to_string()),
                Some("Kubernetes. First part. Second part.".to_string()),
            ]
        );

        let settled = settle(outcomes, &recovery);
        assert_eq!(settled.text, "First part. Second part. Third part.");
        assert_eq!(settled.failed, 0);
        assert_eq!(
            files_in(&parts_dir).len(),
            3,
            "the parts stay until delivery"
        );
        drop(settled.files);
        assert!(files_in(&parts_dir).is_empty(), "delivered, so deleted");
        assert!(files_in(&recovery).is_empty());
    }

    #[tokio::test]
    async fn a_failed_part_goes_to_recovery_and_the_rest_still_joins() {
        let dir = tempfile::tempdir().unwrap();
        let parts_dir = dir.path().join("parts");
        let recovery = dir.path().join("recover");
        let model = Fake::new(&["First part.", "FAIL", "Third part."]);
        let mut parts = TakeParts::<Fake>::new(parts_dir.clone(), "take".into(), None);
        let take = three_sentences();
        parts.feed(&take);
        parts.provide(&model);
        let settled = settle(parts.finish(&take, None).await, &recovery);

        assert_eq!(settled.text, "First part. Third part.");
        assert_eq!(settled.failed, 1);
        assert_eq!(
            model.prompts()[2].as_deref(),
            Some("First part."),
            "a failed part carries nothing"
        );
        let kept = files_in(&recovery);
        assert_eq!(kept.len(), 1);
        assert!(kept[0].ends_with("take-part02.flac"));
        drop(settled.files);
        assert!(files_in(&parts_dir).is_empty());
    }

    #[tokio::test]
    async fn without_a_model_every_part_goes_to_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let parts_dir = dir.path().join("parts");
        let recovery = dir.path().join("recover");
        let mut parts = TakeParts::<Fake>::new(parts_dir.clone(), "take".into(), None);
        let take = three_sentences();
        parts.feed(&take);
        let settled = settle(parts.finish(&take, None).await, &recovery);
        assert_eq!(settled.text, "");
        assert_eq!(settled.failed, 3);
        assert!(settled.files.is_empty());
        assert!(files_in(&parts_dir).is_empty());
        assert_eq!(files_in(&recovery).len(), 3);
    }

    #[tokio::test]
    async fn a_silent_part_needs_no_model_call() {
        let dir = tempfile::tempdir().unwrap();
        let model = Fake::new(&["Hello."]);
        let mut parts = TakeParts::<Fake>::new(dir.path().join("parts"), "t".into(), None);
        let mut take = vec![0.0f32; 30 * RATE];
        take.extend(speech(2.0));
        parts.feed(&take);
        assert!(parts.has_parts(), "the hard cut ends the silence");
        let settled = settle(parts.finish(&take, Some(&model)).await, dir.path());
        assert_eq!(settled.text, "Hello.");
        assert_eq!(model.prompts().len(), 1);
    }

    #[tokio::test]
    async fn a_discarded_take_deletes_its_parts_and_a_kept_one_moves_them() {
        let dir = tempfile::tempdir().unwrap();
        let parts_dir = dir.path().join("parts");
        let recovery = dir.path().join("recover");
        let mut parts = TakeParts::<Fake>::new(parts_dir.clone(), "a".into(), None);
        parts.feed(&three_sentences());
        assert_eq!(files_in(&parts_dir).len(), 2);
        parts.discard();
        assert!(files_in(&parts_dir).is_empty());

        let mut parts = TakeParts::<Fake>::new(parts_dir.clone(), "b".into(), None);
        parts.feed(&three_sentences());
        parts.keep(&recovery);
        assert!(files_in(&parts_dir).is_empty());
        assert_eq!(files_in(&recovery).len(), 2);
    }

    #[test]
    fn texts_join_with_single_spaces() {
        let texts = [" Hello there. ", "", "  ", "How are you?"].map(String::from);
        assert_eq!(join(&texts), "Hello there. How are you?");
        assert_eq!(join(&[]), "");
    }
}
