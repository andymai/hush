//! Keep the microphone awake between takes.
//!
//! The input stream pauses between takes, and WirePlumber then suspends the
//! idle source after `session.suspend-timeout-seconds` (5 s by default). Some
//! USB microphones need most of a second to wake, and the next take loses its
//! first word. `hush setup mic` writes one WirePlumber rule that turns off the
//! suspend for that source only. No stream stays open, so no app records
//! between takes and the privacy indicator stays off.

use crate::audio::take::StartDelay;
use crate::audio::AudioCapture;
use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use serde_json::{Map, Value};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

/// The node property that WirePlumber reads to suspend an idle node.
pub const SUSPEND_KEY: &str = "session.suspend-timeout-seconds";

/// WirePlumber suspends a node without [`SUSPEND_KEY`] after this time.
pub const DEFAULT_SUSPEND_SECS: u64 = 5;

/// The name of the one rule file that Hush owns, without its extension.
pub const RULE_NAME: &str = "51-hush-keep-mic-awake";

/// The command that writes the rule.
pub const FIX: &str = "hush setup mic";

const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
const RESTART_TIMEOUT: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_millis(250);
/// After a restart, WirePlumber gets this many polls (10 s) to show the
/// source again.
const POLLS: u32 = 40;

/// What the wake code asks of the machine. Tests answer with a fake.
#[async_trait]
pub trait System: Sync {
    /// The output of `pw-dump`.
    async fn dump(&self) -> Result<String>;
    /// The rule format of the WirePlumber on this machine.
    async fn wireplumber(&self) -> Option<Format>;
    /// `systemctl --user restart wireplumber`
    async fn restart_wireplumber(&self) -> Result<()>;
    async fn pause(&self, time: Duration);
}

/// This machine.
pub struct Host;

#[async_trait]
impl System for Host {
    async fn dump(&self) -> Result<String> {
        let output = run("pw-dump", &[], COMMAND_TIMEOUT).await?;
        Ok(String::from_utf8_lossy(&output).into_owned())
    }

    async fn wireplumber(&self) -> Option<Format> {
        if let Ok(output) = run("wireplumber", &["--version"], COMMAND_TIMEOUT).await {
            if let Some(format) = Format::from_version(&String::from_utf8_lossy(&output)) {
                return Some(format);
            }
        }
        Format::from_system_files(Path::new("/usr/share/wireplumber"))
    }

    async fn restart_wireplumber(&self) -> Result<()> {
        run(
            "systemctl",
            &["--user", "restart", "wireplumber"],
            RESTART_TIMEOUT,
        )
        .await
        .map(drop)
    }

    async fn pause(&self, time: Duration) {
        tokio::time::sleep(time).await;
    }
}

/// The standard output of `program`. A failure names the first line of its
/// error output.
async fn run(program: &str, args: &[&str], timeout: Duration) -> Result<Vec<u8>> {
    let mut command = Command::new(program);
    command.args(args).stdin(Stdio::null()).kill_on_drop(true);
    let output = tokio::time::timeout(timeout, command.output())
        .await
        .map_err(|_| anyhow!("{} gave no answer in {} s", program, timeout.as_secs()))?
        .with_context(|| format!("Could not run {}", program))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("no message");
        bail!(
            "{} {} failed ({}): {}",
            program,
            args.join(" "),
            output.status,
            reason
        );
    }
    Ok(output.stdout)
}

/// The rule format of a WirePlumber version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// WirePlumber 0.5 and later: SPA-JSON in `wireplumber.conf.d`
    Conf,
    /// WirePlumber 0.4: Lua in `main.lua.d`
    Lua,
}

impl Format {
    pub const ALL: [Format; 2] = [Format::Conf, Format::Lua];

    /// From the output of `wireplumber --version`, which names the library
    /// that it was compiled with and the one that it linked.
    pub fn from_version(text: &str) -> Option<Self> {
        let linked = text
            .lines()
            .find(|line| line.contains("Linked with"))
            .and_then(version_in);
        let (major, minor) = linked.or_else(|| text.lines().find_map(version_in))?;
        match (major, minor) {
            (0, 4) => Some(Format::Lua),
            (0, minor) if minor >= 5 => Some(Format::Conf),
            (major, _) if major >= 1 => Some(Format::Conf),
            _ => None,
        }
    }

    /// From the configuration that WirePlumber installs in `share`, such as
    /// `/usr/share/wireplumber`, when `wireplumber --version` gives no answer.
    pub fn from_system_files(share: &Path) -> Option<Self> {
        if share.join("wireplumber.conf").is_file() {
            Some(Format::Conf)
        } else if share.join("main.lua.d").is_dir() {
            Some(Format::Lua)
        } else {
            None
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Format::Conf => "WirePlumber 0.5",
            Format::Lua => "WirePlumber 0.4",
        }
    }

    /// Hush's rule file of this format. `config_dir` is the folder that holds
    /// the `wireplumber` folder, `$XDG_CONFIG_HOME` or `~/.config`.
    pub fn rule_path(self, config_dir: &Path) -> PathBuf {
        let wireplumber = config_dir.join("wireplumber");
        match self {
            Format::Conf => wireplumber
                .join("wireplumber.conf.d")
                .join(format!("{}.conf", RULE_NAME)),
            Format::Lua => wireplumber
                .join("main.lua.d")
                .join(format!("{}.lua", RULE_NAME)),
        }
    }

    /// The text of the rule file that keeps `source` awake.
    pub fn rule(self, source: &Source) -> Result<String> {
        let matcher = self.matcher(source)?;
        let label: String = source.label.chars().filter(|c| !c.is_control()).collect();
        let notes = [
            format!("Written by `hush setup mic` for {}.", label),
            "WirePlumber does not suspend this microphone, so a take keeps its first word."
                .to_string(),
            "`hush setup mic --undo` removes this file.".to_string(),
        ];
        let text = match self {
            Format::Conf => format!(
                "# {}\n# {}\n# {}\nmonitor.alsa.rules = [\n  {{\n    matches = [\n      {{\n        \
                 {}\n      }}\n    ]\n    actions = {{\n      update-props = {{\n        {} = 0\n      \
                 }}\n    }}\n  }}\n]\n",
                notes[0], notes[1], notes[2], matcher, SUSPEND_KEY
            ),
            Format::Lua => format!(
                "-- {}\n-- {}\n-- {}\ntable.insert(alsa_monitor.rules, {{\n  matches = {{\n    {{\n      \
                 {}\n    }},\n  }},\n  apply_properties = {{\n    [\"{}\"] = 0,\n  }},\n}})\n",
                notes[0], notes[1], notes[2], matcher, SUSPEND_KEY
            ),
        };
        Ok(text)
    }

    /// The line of the rule that matches `source` by its node name without
    /// the profile, so that the rule still applies after a profile change.
    /// A name without a known profile must match as a whole.
    fn matcher(self, source: &Source) -> Result<String> {
        if !is_plain(&source.name) {
            bail!(
                "Hush writes no rule for the node name {:?}, because it has characters other \
                 than letters, digits, '.', '_', and '-'",
                source.name
            );
        }
        // A bracket matches a literal dot in both regex engines that WirePlumber
        // 0.5 can use, and needs no escape in SPA-JSON.
        Ok(match (self, source.stem()) {
            (Format::Conf, Some(stem)) => {
                format!("node.name = \"~^{}[.]\"", stem.replace('.', "[.]"))
            },
            (Format::Conf, None) => format!("node.name = \"{}\"", source.name),
            (Format::Lua, Some(stem)) => {
                format!("{{ \"node.name\", \"matches\", \"{}.*\" }},", stem)
            },
            (Format::Lua, None) => format!("{{ \"node.name\", \"equals\", \"{}\" }},", source.name),
        })
    }
}

/// The first number pair `major.minor` in a line, such as `0.5` in
/// `Linked with libwireplumber 0.5.19`.
fn version_in(line: &str) -> Option<(u32, u32)> {
    line.split_whitespace().find_map(|word| {
        let mut parts = word.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        Some((major, minor))
    })
}

fn is_plain(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// When WirePlumber suspends an idle source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sleep {
    Never,
    /// After this many seconds without a stream
    After(u64),
}

/// The kind of PipeWire node, which decides the rule that applies to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A sound card that WirePlumber's ALSA monitor made
    Alsa,
    /// A Bluetooth headset
    Bluetooth,
    /// A virtual node, such as a loopback or a filter
    Other,
}

/// A PipeWire source node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// `node.name`
    pub name: String,
    /// `node.nick`, else `node.description`, else the name
    pub label: String,
    /// `device.profile.name`, the last part of an ALSA node name
    pub profile: Option<String>,
    /// `object.serial`, new for each node that PipeWire makes
    pub serial: Option<u64>,
    pub kind: Kind,
    pub sleep: Sleep,
}

impl Source {
    /// The node name without `.` and the profile, when it ends with them.
    fn stem(&self) -> Option<&str> {
        let profile = self.profile.as_deref().filter(|p| !p.is_empty())?;
        self.name
            .strip_suffix(profile)?
            .strip_suffix('.')
            .filter(|stem| !stem.is_empty())
    }

    fn from_props(props: &Map<String, Value>) -> Option<Self> {
        let name = text(props.get("node.name"))?;
        let label = ["node.nick", "node.description"]
            .iter()
            .filter_map(|key| text(props.get(*key)))
            .find(|label| !label.trim().is_empty())
            .unwrap_or_else(|| name.clone());
        let kind = match text(props.get("device.api")).as_deref() {
            Some("alsa") => Kind::Alsa,
            Some("bluez5") => Kind::Bluetooth,
            _ if name.starts_with("alsa_") => Kind::Alsa,
            _ if name.starts_with("bluez_") => Kind::Bluetooth,
            _ => Kind::Other,
        };
        Some(Self {
            label,
            profile: text(props.get("device.profile.name")),
            serial: text(props.get("object.serial")).and_then(|serial| serial.parse().ok()),
            kind,
            sleep: sleep_of(props.get(SUSPEND_KEY)),
            name,
        })
    }
}

/// A property value as text: pw-dump prints numbers and booleans bare.
fn text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

/// WirePlumber reads the property as a number and uses 5 s without one.
fn sleep_of(value: Option<&Value>) -> Sleep {
    let seconds = value.and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_str()?.trim().parse().ok())
    });
    match seconds {
        Some(0.0) => Sleep::Never,
        Some(seconds) if seconds > 0.0 => Sleep::After(seconds.ceil() as u64),
        _ => Sleep::After(DEFAULT_SUSPEND_SECS),
    }
}

/// The node that a stream of Hush records from.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Wanted {
    /// The default source in the `default` metadata
    Default,
    /// A node name, serial, or id, as `PIPEWIRE_NODE` holds it
    Named(String),
}

impl fmt::Display for Wanted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Wanted::Default => f.write_str("for the default source"),
            Wanted::Named(name) => f.write_str(name),
        }
    }
}

/// The objects of the first JSON array in `pw-dump` output. pw-dump 1.0 can
/// print a second array with the changes that came during the dump.
fn parse_dump(text: &str) -> Result<Vec<Value>> {
    let first = serde_json::Deserializer::from_str(text)
        .into_iter::<Value>()
        .next()
        .context("pw-dump printed nothing")??;
    match first {
        Value::Array(objects) => Ok(objects),
        _ => bail!("pw-dump printed no array"),
    }
}

fn default_source(objects: &[Value]) -> Option<String> {
    objects
        .iter()
        .filter(|object| {
            object["type"] == "PipeWire:Interface:Metadata"
                && object["props"]["metadata.name"] == "default"
        })
        .flat_map(|object| object["metadata"].as_array().into_iter().flatten())
        .find(|entry| entry["key"] == "default.audio.source")
        .and_then(|entry| match &entry["value"] {
            Value::Object(value) => value.get("name")?.as_str().map(str::to_owned),
            Value::String(json) => serde_json::from_str::<Value>(json)
                .ok()?
                .get("name")?
                .as_str()
                .map(str::to_owned),
            _ => None,
        })
}

fn find(objects: &[Value], wanted: &Wanted) -> Option<Source> {
    let (name, by_number) = match wanted {
        Wanted::Default => (default_source(objects)?, false),
        Wanted::Named(name) => (name.clone(), true),
    };
    objects
        .iter()
        .filter(|object| object["type"] == "PipeWire:Interface:Node")
        .find_map(|object| {
            let props = object["info"]["props"].as_object()?;
            let named = text(props.get("node.name")).as_deref() == Some(name.as_str());
            let numbered = by_number
                && (text(props.get("object.serial")).as_deref() == Some(name.as_str())
                    || text(object.get("id")).as_deref() == Some(name.as_str()));
            if named || numbered {
                Source::from_props(props)
            } else {
                None
            }
        })
}

/// Whether Hush reads the ALSA PCM through the sound server, which picks the
/// source. A card PCM such as `front:CARD=Microphone,DEV=0` goes past it.
pub fn through_sound_server(pcm: &str) -> bool {
    let plugin = pcm.split(':').next().unwrap_or(pcm);
    matches!(plugin, "default" | "pipewire" | "pulse")
}

/// What Hush found out about the source of its takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe {
    /// No input device matches the `audio.device` setting
    NoDevice,
    /// Hush reads this ALSA device past PipeWire
    Direct(String),
    /// `pw-dump` gave no answer: no PipeWire, or no PipeWire tools
    NoPipeWire,
    /// PipeWire runs, but has no such node; the text names the node
    Missing(String),
    Found(Source),
}

/// The source that a take of Hush records from, when Hush reads `pcm`
/// through the sound server: the node that `pipewire_node` names, as
/// `PIPEWIRE_NODE` does for the PipeWire ALSA plugin, else the default
/// source.
pub async fn probe(system: &impl System, pcm: Option<&str>, pipewire_node: Option<&str>) -> Probe {
    let Some(pcm) = pcm else {
        return Probe::NoDevice;
    };
    if !through_sound_server(pcm) {
        return Probe::Direct(pcm.to_string());
    }
    let wanted = match pipewire_node.map(str::trim).filter(|node| !node.is_empty()) {
        Some(node) => Wanted::Named(node.to_string()),
        None => Wanted::Default,
    };
    let Ok(objects) = system.dump().await.and_then(|text| parse_dump(&text)) else {
        return Probe::NoPipeWire;
    };
    match find(&objects, &wanted) {
        Some(source) => Probe::Found(source),
        None => Probe::Missing(wanted.to_string()),
    }
}

/// [`probe`] on this machine, for the `audio.device` setting.
pub async fn look(device: Option<&str>) -> Probe {
    let pcm = AudioCapture::input_pcm(device);
    let pipewire_node = std::env::var("PIPEWIRE_NODE").ok();
    probe(&Host, pcm.as_deref(), pipewire_node.as_deref()).await
}

/// `$XDG_CONFIG_HOME`, else `~/.config`: the folder of the `wireplumber`
/// folder.
pub fn config_dir() -> Result<PathBuf> {
    dirs::config_dir().context("Hush finds no configuration folder ($XDG_CONFIG_HOME or ~/.config)")
}

/// Hush's rule files under `config_dir`.
pub fn rules_in(config_dir: &Path) -> Vec<PathBuf> {
    Format::ALL
        .iter()
        .map(|format| format.rule_path(config_dir))
        .filter(|path| path.is_file())
        .collect()
}

/// Whether a rule file of Hush under `config_dir` matches `source`.
pub fn rule_names(config_dir: &Path, source: &Source) -> bool {
    Format::ALL.iter().any(|format| {
        let Ok(matcher) = format.matcher(source) else {
            return false;
        };
        fs::read_to_string(format.rule_path(config_dir)).is_ok_and(|text| text.contains(&matcher))
    })
}

/// What `hush doctor` and the settings window say about the wake of the
/// microphone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub warn: bool,
    pub detail: String,
    /// The next step for a warning
    pub fix: Option<String>,
    /// [`FIX`] removes the warning
    pub fixable: bool,
}

impl Finding {
    fn fine(detail: impl Into<String>) -> Self {
        Self {
            warn: false,
            detail: detail.into(),
            fix: None,
            fixable: false,
        }
    }

    fn warn(detail: impl Into<String>, fix: impl Into<String>, fixable: bool) -> Self {
        Self {
            warn: true,
            detail: detail.into(),
            fix: Some(fix.into()),
            fixable,
        }
    }
}

/// The finding for the source, Hush's rule for it (`hush_rule`), and the
/// start delay of recent takes, `None` without a daemon or a take.
pub fn finding(probe: &Probe, hush_rule: bool, delay: Option<StartDelay>) -> Finding {
    let source =
        match probe {
            Probe::NoDevice => {
                return Finding::fine("Hush finds no input device, so there is nothing to check")
            },
            Probe::Direct(pcm) => {
                return Finding::fine(format!(
                    "Hush reads {} through ALSA, past PipeWire, so WirePlumber does not suspend it",
                    pcm
                ))
            },
            Probe::NoPipeWire => return Finding::fine(
                "PipeWire does not answer (PulseAudio or plain ALSA), so there is nothing to check",
            ),
            Probe::Missing(node) => {
                return Finding::fine(format!(
                    "PipeWire shows no node {}, so there is nothing to check",
                    node
                ))
            },
            Probe::Found(source) => source,
        };
    let label = &source.label;
    let seconds = match source.sleep {
        Sleep::Never if hush_rule => {
            return Finding::fine(format!(
                "{} stays awake between takes (the WirePlumber rule of Hush)",
                label
            ))
        },
        Sleep::Never => return Finding::fine(format!("{} stays awake between takes", label)),
        Sleep::After(seconds) => seconds,
    };
    let sleeps = format!("{} sleeps after {} s without a take", label, seconds);
    match (source.kind, delay) {
        (_, Some(delay)) if !delay.is_slow() => Finding::fine(format!(
            "{} and wakes in {} ms or less",
            sleeps, delay.worst_ms
        )),
        (Kind::Alsa, Some(delay)) => Finding::warn(
            format!(
                "{} and needs up to {} ms to wake, which can cut off the first word",
                sleeps, delay.worst_ms
            ),
            FIX,
            true,
        ),
        (Kind::Alsa, None) => Finding::warn(
            format!("{}; no take measured its wake time yet", sleeps),
            FIX,
            true,
        ),
        (Kind::Bluetooth, Some(delay)) => Finding::warn(
            format!("{} and needs up to {} ms to wake", sleeps, delay.worst_ms),
            "Hush does not keep a Bluetooth microphone awake, because an open headset link \
             lowers the sound quality; a wired microphone wakes faster",
            false,
        ),
        (Kind::Bluetooth, None) => Finding::fine(format!(
            "{}; Hush does not keep a Bluetooth microphone awake",
            sleeps
        )),
        (Kind::Other, Some(delay)) => Finding::warn(
            format!("{} and needs up to {} ms to wake", sleeps, delay.worst_ms),
            format!(
                "Hush writes rules only for ALSA microphones; set {} = 0 where {} is defined",
                SUSPEND_KEY, source.name
            ),
            false,
        ),
        (Kind::Other, None) => Finding::fine(format!(
            "{}; it is not an ALSA device, so Hush does not change it",
            sleeps
        )),
    }
}

/// What `hush setup mic` does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// The source stays awake already; `by_hush` when Hush's rule does it
    Awake {
        source: Source,
        by_hush: bool,
    },
    Write(Change),
}

/// A rule file to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub source: Source,
    pub format: Format,
    pub path: PathBuf,
    pub text: String,
    /// Hush's rule file of the other format, from before a WirePlumber update
    pub stale: Vec<PathBuf>,
}

/// Decide what keeps the source of `probe` awake. Nothing changes yet.
pub async fn plan(system: &impl System, config_dir: &Path, probe: Probe) -> Result<Plan> {
    let source = found(probe)?;
    match source.kind {
        Kind::Alsa => {},
        Kind::Bluetooth => bail!(
            "{} is a Bluetooth microphone. Hush does not keep it awake, because an open headset \
             link lowers the sound quality and uses the battery.",
            source.label
        ),
        Kind::Other => bail!(
            "{} ({}) is not an ALSA device, so a WirePlumber rule for ALSA devices does not \
             apply to it. Set {} = 0 where it is defined.",
            source.label,
            source.name,
            SUSPEND_KEY
        ),
    }
    if source.sleep == Sleep::Never {
        let by_hush = rule_names(config_dir, &source);
        return Ok(Plan::Awake { source, by_hush });
    }
    let format = system.wireplumber().await.context(
        "Hush finds no WirePlumber 0.4 or later, the PipeWire session manager that reads the rule",
    )?;
    let text = format.rule(&source)?;
    let stale = Format::ALL
        .iter()
        .filter(|other| **other != format)
        .map(|other| other.rule_path(config_dir))
        .filter(|path| path.exists())
        .collect();
    Ok(Plan::Write(Change {
        path: format.rule_path(config_dir),
        source,
        format,
        text,
        stale,
    }))
}

fn found(probe: Probe) -> Result<Source> {
    match probe {
        Probe::Found(source) => Ok(source),
        Probe::NoDevice => bail!(
            "Hush finds no input device for the audio.device setting. `hush status --devices` \
             lists the devices."
        ),
        Probe::Direct(pcm) => bail!(
            "Hush reads {} through ALSA, past PipeWire. WirePlumber does not suspend it, so a \
             rule does not help.",
            pcm
        ),
        Probe::NoPipeWire => bail!(
            "PipeWire does not answer (pw-dump failed). The rule is for PipeWire with WirePlumber."
        ),
        Probe::Missing(node) => bail!(
            "PipeWire shows no node {}. Choose a microphone in the sound settings of your desktop.",
            node
        ),
    }
}

/// Write the rule, restart WirePlumber, and wait for the source to come
/// back. Returns the source after the restart, which stays awake.
pub async fn apply(system: &impl System, change: &Change) -> Result<Source> {
    write_rule(&change.path, &change.text)?;
    for stale in &change.stale {
        remove(stale)?;
    }
    system.restart_wireplumber().await.with_context(|| {
        format!(
            "The rule is in {}, but WirePlumber did not restart. Log out and in, or run \
             systemctl --user restart wireplumber",
            change.path.display()
        )
    })?;
    let source = back(system, &change.source).await?;
    if let Sleep::After(seconds) = source.sleep {
        bail!(
            "WirePlumber restarted, but {} still sleeps after {} s. Look at {}",
            source.label,
            seconds,
            change.path.display()
        );
    }
    Ok(source)
}

/// What `hush setup mic --undo` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Undone {
    /// Hush wrote no rule, so nothing changed
    Nothing,
    /// The rule files are gone, and WirePlumber restarted. `source` is the
    /// microphone after the restart, when the probe found it before.
    Removed {
        files: Vec<PathBuf>,
        source: Option<Source>,
    },
}

/// Remove Hush's rule files, restart WirePlumber, and wait for the source
/// of `probe` to come back.
pub async fn undo(system: &impl System, config_dir: &Path, probe: Probe) -> Result<Undone> {
    let files = rules_in(config_dir);
    if files.is_empty() {
        return Ok(Undone::Nothing);
    }
    for file in &files {
        remove(file)?;
    }
    system.restart_wireplumber().await.context(
        "Hush removed its rule, but WirePlumber did not restart. Log out and in, or run \
         systemctl --user restart wireplumber",
    )?;
    let source = match probe {
        Probe::Found(before) => Some(back(system, &before).await?),
        _ => None,
    };
    Ok(Undone::Removed { files, source })
}

/// The node of `before` after a WirePlumber restart: the same name with a
/// new serial, because the old node can still show for a moment.
async fn back(system: &impl System, before: &Source) -> Result<Source> {
    let wanted = Wanted::Named(before.name.clone());
    for _ in 0..POLLS {
        system.pause(POLL).await;
        let Ok(objects) = system.dump().await.and_then(|text| parse_dump(&text)) else {
            continue;
        };
        if let Some(now) = find(&objects, &wanted) {
            if before.serial.is_none() || now.serial != before.serial {
                return Ok(now);
            }
        }
    }
    bail!(
        "{} did not come back in {} s after WirePlumber restarted",
        before.label,
        (POLL * POLLS).as_secs()
    )
}

fn write_rule(path: &Path, text: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    }
    crate::config::transfer::write_atomic(path, text.as_bytes(), true)
}

fn remove(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("Failed to remove {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;
    use std::collections::VecDeque;

    const YETI: &str =
        "alsa_input.usb-Blue_Microphones_Yeti_Stereo_Microphone_REV8-00.iec958-stereo";

    fn node(id: u64, serial: u64, name: &str, extra: Value) -> Value {
        let mut props = serde_json::json!({
            "node.name": name,
            "object.serial": serial,
            "media.class": "Audio/Source",
        });
        if let (Some(props), Value::Object(extra)) = (props.as_object_mut(), extra) {
            props.extend(extra);
        }
        serde_json::json!({
            "id": id,
            "type": "PipeWire:Interface:Node",
            "info": { "state": "suspended", "props": props },
        })
    }

    fn yeti(serial: u64, suspend: Option<Value>) -> Value {
        let mut extra = serde_json::json!({
            "device.api": "alsa",
            "device.profile.name": "iec958-stereo",
            "node.nick": "Yeti Stereo Microphone",
            "node.description": "Yeti Stereo Microphone Digital Stereo (IEC958)",
        });
        if let (Some(extra), Some(suspend)) = (extra.as_object_mut(), suspend) {
            extra.insert(SUSPEND_KEY.to_string(), suspend);
        }
        node(85, serial, YETI, extra)
    }

    fn defaults(source: &str) -> Value {
        serde_json::json!({
            "id": 33,
            "type": "PipeWire:Interface:Metadata",
            "props": { "metadata.name": "default" },
            "metadata": [
                { "subject": 0, "key": "default.configured.audio.source", "type": "Spa:String:JSON", "value": { "name": "elsewhere" } },
                { "subject": 0, "key": "default.audio.source", "type": "Spa:String:JSON", "value": { "name": source } },
            ],
        })
    }

    fn dump(objects: Vec<Value>) -> String {
        serde_json::to_string_pretty(&Value::Array(objects)).unwrap()
    }

    /// A machine with a script of `pw-dump` outputs; the last one repeats.
    struct Fake {
        dumps: Mutex<VecDeque<Result<String, String>>>,
        format: Option<Format>,
        restart: Result<(), String>,
        restarts: Mutex<u32>,
    }

    impl Fake {
        fn new(dumps: Vec<Result<String, String>>) -> Self {
            Self {
                dumps: Mutex::new(dumps.into()),
                format: Some(Format::Conf),
                restart: Ok(()),
                restarts: Mutex::new(0),
            }
        }

        fn restarts(&self) -> u32 {
            *self.restarts.lock()
        }
    }

    #[async_trait]
    impl System for Fake {
        async fn dump(&self) -> Result<String> {
            let mut dumps = self.dumps.lock();
            let next = if dumps.len() > 1 {
                dumps.pop_front()
            } else {
                dumps.front().cloned()
            };
            next.unwrap_or_else(|| Err("no dump".into()))
                .map_err(|e| anyhow!(e))
        }

        async fn wireplumber(&self) -> Option<Format> {
            self.format
        }

        async fn restart_wireplumber(&self) -> Result<()> {
            *self.restarts.lock() += 1;
            self.restart.clone().map_err(|e| anyhow!(e))
        }

        async fn pause(&self, _time: Duration) {}
    }

    fn found(probe: Probe) -> Source {
        match probe {
            Probe::Found(source) => source,
            other => panic!("no source: {other:?}"),
        }
    }

    #[tokio::test]
    async fn the_default_source_and_its_suspend_come_from_pw_dump() {
        let fake = Fake::new(vec![Ok(dump(vec![
            node(
                86,
                10,
                "alsa_input.usb-Audient_EVO4-00.pro-input-0",
                serde_json::json!({}),
            ),
            yeti(8356, None),
            defaults(YETI),
        ]))]);
        let source = found(probe(&fake, Some("default"), None).await);
        assert_eq!(source.name, YETI);
        assert_eq!(source.label, "Yeti Stereo Microphone");
        assert_eq!(source.kind, Kind::Alsa);
        assert_eq!(source.serial, Some(8356));
        assert_eq!(
            source.sleep,
            Sleep::After(DEFAULT_SUSPEND_SECS),
            "WirePlumber's default without the property"
        );

        for (value, sleep) in [
            (serde_json::json!(0), Sleep::Never),
            (serde_json::json!("0"), Sleep::Never),
            (serde_json::json!(3), Sleep::After(3)),
            (serde_json::json!("12"), Sleep::After(12)),
            (
                serde_json::json!("soon"),
                Sleep::After(DEFAULT_SUSPEND_SECS),
            ),
        ] {
            let fake = Fake::new(vec![Ok(dump(vec![
                yeti(1, Some(value.clone())),
                defaults(YETI),
            ]))]);
            assert_eq!(
                found(probe(&fake, Some("pipewire"), None).await).sleep,
                sleep,
                "{value}"
            );
        }
    }

    #[tokio::test]
    async fn pipewire_node_picks_the_node_by_name_serial_or_id() {
        let objects = vec![
            yeti(8356, None),
            node(
                120,
                9001,
                "hush-e2e-mic-42",
                serde_json::json!({"node.description": "Hush e2e microphone"}),
            ),
            defaults(YETI),
        ];
        for wanted in ["hush-e2e-mic-42", "9001", "120"] {
            let fake = Fake::new(vec![Ok(dump(objects.clone()))]);
            let source = found(probe(&fake, Some("default"), Some(wanted)).await);
            assert_eq!(source.name, "hush-e2e-mic-42", "{wanted}");
            assert_eq!(source.label, "Hush e2e microphone");
            assert_eq!(source.kind, Kind::Other);
        }
        let fake = Fake::new(vec![Ok(dump(objects))]);
        assert_eq!(
            probe(&fake, Some("default"), Some("gone")).await,
            Probe::Missing("gone".into())
        );
    }

    #[tokio::test]
    async fn a_card_pcm_or_no_pipewire_skips_the_check() {
        let fake = Fake::new(vec![Err("Connection refused".into())]);
        assert_eq!(
            probe(&fake, Some("front:CARD=Microphone,DEV=0"), None).await,
            Probe::Direct("front:CARD=Microphone,DEV=0".into())
        );
        assert_eq!(probe(&fake, None, None).await, Probe::NoDevice);
        assert_eq!(probe(&fake, Some("default"), None).await, Probe::NoPipeWire);
        let garbage = Fake::new(vec![Ok("not json".into())]);
        assert_eq!(
            probe(&garbage, Some("default"), None).await,
            Probe::NoPipeWire
        );
        let no_default = Fake::new(vec![Ok(dump(vec![yeti(1, None)]))]);
        assert_eq!(
            probe(&no_default, Some("default"), None).await,
            Probe::Missing("for the default source".into())
        );
        assert!(through_sound_server("default"));
        assert!(through_sound_server("pipewire"));
        assert!(through_sound_server("pulse"));
        assert!(!through_sound_server("sysdefault:CARD=Microphone"));
        assert!(!through_sound_server("hw:CARD=0,DEV=0"));
    }

    #[test]
    fn a_second_array_from_pw_dump_and_a_metadata_string_are_read() {
        let text = format!(
            "{}\n{}",
            dump(vec![
                yeti(1, None),
                serde_json::json!({
                    "id": 33,
                    "type": "PipeWire:Interface:Metadata",
                    "props": { "metadata.name": "default" },
                    "metadata": [{ "key": "default.audio.source", "value": format!("{{\"name\":\"{YETI}\"}}") }],
                }),
            ]),
            dump(vec![])
        );
        let objects = parse_dump(&text).unwrap();
        assert_eq!(default_source(&objects).as_deref(), Some(YETI));
    }

    #[test]
    fn the_version_picks_the_rule_format() {
        let new =
            "wireplumber\nCompiled with libwireplumber 0.5.19\nLinked with libwireplumber 0.5.19\n";
        assert_eq!(Format::from_version(new), Some(Format::Conf));
        let old =
            "wireplumber\nCompiled with libwireplumber 0.4.17\nLinked with libwireplumber 0.4.17\n";
        assert_eq!(Format::from_version(old), Some(Format::Lua));
        let mixed =
            "wireplumber\nCompiled with libwireplumber 0.4.17\nLinked with libwireplumber 0.5.2\n";
        assert_eq!(
            Format::from_version(mixed),
            Some(Format::Conf),
            "the linked library counts"
        );
        assert_eq!(
            Format::from_version("wireplumber 1.0.1"),
            Some(Format::Conf)
        );
        assert_eq!(Format::from_version("libwireplumber 0.3.96"), None);
        assert_eq!(Format::from_version("Unknown option --version"), None);

        let share = tempfile::tempdir().unwrap();
        assert_eq!(Format::from_system_files(share.path()), None);
        fs::create_dir(share.path().join("main.lua.d")).unwrap();
        assert_eq!(Format::from_system_files(share.path()), Some(Format::Lua));
        fs::write(share.path().join("wireplumber.conf"), "").unwrap();
        assert_eq!(Format::from_system_files(share.path()), Some(Format::Conf));
    }

    fn yeti_source(sleep: Sleep) -> Source {
        Source {
            name: YETI.to_string(),
            label: "Yeti Stereo Microphone".to_string(),
            profile: Some("iec958-stereo".to_string()),
            serial: Some(8356),
            kind: Kind::Alsa,
            sleep,
        }
    }

    #[test]
    fn the_rule_matches_the_node_without_its_profile() {
        let source = yeti_source(Sleep::After(5));
        let conf = Format::Conf.rule(&source).unwrap();
        assert_eq!(
            conf,
            "# Written by `hush setup mic` for Yeti Stereo Microphone.\n\
             # WirePlumber does not suspend this microphone, so a take keeps its first word.\n\
             # `hush setup mic --undo` removes this file.\n\
             monitor.alsa.rules = [\n  {\n    matches = [\n      {\n        \
             node.name = \"~^alsa_input[.]usb-Blue_Microphones_Yeti_Stereo_Microphone_REV8-00[.]\"\n      \
             }\n    ]\n    actions = {\n      update-props = {\n        \
             session.suspend-timeout-seconds = 0\n      }\n    }\n  }\n]\n"
        );
        let lua = Format::Lua.rule(&source).unwrap();
        assert_eq!(
            lua,
            "-- Written by `hush setup mic` for Yeti Stereo Microphone.\n\
             -- WirePlumber does not suspend this microphone, so a take keeps its first word.\n\
             -- `hush setup mic --undo` removes this file.\n\
             table.insert(alsa_monitor.rules, {\n  matches = {\n    {\n      \
             { \"node.name\", \"matches\", \"alsa_input.usb-Blue_Microphones_Yeti_Stereo_Microphone_REV8-00.*\" },\n    \
             },\n  },\n  apply_properties = {\n    [\"session.suspend-timeout-seconds\"] = 0,\n  },\n})\n"
        );

        let pci = Source {
            name: "alsa_input.pci-0000_00_1f.3.analog-stereo".to_string(),
            profile: Some("analog-stereo".to_string()),
            ..source.clone()
        };
        assert!(Format::Conf
            .rule(&pci)
            .unwrap()
            .contains("\"~^alsa_input[.]pci-0000_00_1f[.]3[.]\""));

        let no_profile = Source {
            profile: None,
            ..source.clone()
        };
        assert!(Format::Conf
            .rule(&no_profile)
            .unwrap()
            .contains(&format!("node.name = \"{YETI}\"")));
        assert!(Format::Lua
            .rule(&no_profile)
            .unwrap()
            .contains(&format!("{{ \"node.name\", \"equals\", \"{YETI}\" }}")));

        let odd = Source {
            name: "alsa_input.\"x\"".to_string(),
            ..source
        };
        assert!(Format::Conf.rule(&odd).is_err());
    }

    #[test]
    fn the_rule_files_live_in_the_wireplumber_folders() {
        let dir = Path::new("/home/me/.config");
        assert_eq!(
            Format::Conf.rule_path(dir),
            Path::new(
                "/home/me/.config/wireplumber/wireplumber.conf.d/51-hush-keep-mic-awake.conf"
            )
        );
        assert_eq!(
            Format::Lua.rule_path(dir),
            Path::new("/home/me/.config/wireplumber/main.lua.d/51-hush-keep-mic-awake.lua")
        );
    }

    fn delay(worst_ms: u64) -> Option<StartDelay> {
        Some(StartDelay {
            last_ms: 20,
            worst_ms,
            takes: 5,
        })
    }

    #[test]
    fn a_sleeping_alsa_source_warns_when_slow_or_unmeasured() {
        let sleeps = Probe::Found(yeti_source(Sleep::After(5)));
        let slow = finding(&sleeps, false, delay(650));
        assert!(slow.warn && slow.fixable);
        assert_eq!(
            slow.detail,
            "Yeti Stereo Microphone sleeps after 5 s without a take and needs up to 650 ms to \
             wake, which can cut off the first word"
        );
        assert_eq!(slow.fix.as_deref(), Some(FIX));

        let unknown = finding(&sleeps, false, None);
        assert!(unknown.warn && unknown.fixable);
        assert!(
            unknown.detail.contains("no take measured"),
            "{}",
            unknown.detail
        );

        let quick = finding(&sleeps, false, delay(200));
        assert!(!quick.warn);
        assert_eq!(
            quick.detail,
            "Yeti Stereo Microphone sleeps after 5 s without a take and wakes in 200 ms or less"
        );
    }

    #[test]
    fn an_awake_source_is_fine_whoever_wrote_the_rule() {
        let awake = Probe::Found(yeti_source(Sleep::Never));
        let theirs = finding(&awake, false, delay(900));
        assert!(!theirs.warn, "the delay came before the rule");
        assert_eq!(
            theirs.detail,
            "Yeti Stereo Microphone stays awake between takes"
        );
        let ours = finding(&awake, true, None);
        assert!(ours.detail.ends_with("(the WirePlumber rule of Hush)"));
    }

    #[test]
    fn bluetooth_and_virtual_sources_get_no_rule() {
        let headset = Source {
            name: "bluez_input.AC_80_0A_00_00_01.0".to_string(),
            label: "WH-1000XM4".to_string(),
            profile: None,
            serial: None,
            kind: Kind::Bluetooth,
            sleep: Sleep::After(5),
        };
        let slow = finding(&Probe::Found(headset.clone()), false, delay(1500));
        assert!(slow.warn && !slow.fixable);
        assert!(slow.fix.unwrap().contains("Bluetooth"));
        assert!(!finding(&Probe::Found(headset), false, None).warn);

        let virtual_mic = Source {
            name: "hush-e2e-mic-42".to_string(),
            label: "Hush e2e microphone".to_string(),
            kind: Kind::Other,
            ..yeti_source(Sleep::After(5))
        };
        let quiet = finding(&Probe::Found(virtual_mic.clone()), false, None);
        assert!(!quiet.warn);
        assert!(quiet.detail.contains("not an ALSA device"));
        let slow = finding(&Probe::Found(virtual_mic), false, delay(800));
        assert!(slow.warn && !slow.fixable);

        for probe in [
            Probe::NoDevice,
            Probe::Direct("front:CARD=Microphone,DEV=0".into()),
            Probe::NoPipeWire,
            Probe::Missing("for the default source".into()),
        ] {
            let skip = finding(&probe, false, delay(900));
            assert!(!skip.warn, "{probe:?}");
            assert!(
                skip.detail.contains("nothing to check") || skip.detail.contains("past PipeWire")
            );
        }
    }

    #[tokio::test]
    async fn setup_writes_the_rule_restarts_and_waits_for_the_awake_node() {
        let config = tempfile::tempdir().unwrap();
        let before = dump(vec![yeti(100, None), defaults(YETI)]);
        let fake = Fake::new(vec![
            Ok(before.clone()),
            Err("WirePlumber restarts".into()),
            Ok(dump(vec![yeti(100, None)])),
            Ok(dump(vec![
                yeti(101, Some(serde_json::json!(0))),
                defaults(YETI),
            ])),
        ]);
        let probe = probe(&fake, Some("default"), None).await;
        let Plan::Write(change) = plan(&fake, config.path(), probe).await.unwrap() else {
            panic!("the source sleeps, so the plan writes a rule");
        };
        assert_eq!(change.format, Format::Conf);
        assert_eq!(change.path, Format::Conf.rule_path(config.path()));
        assert!(change.stale.is_empty());
        assert!(!change.path.exists(), "a plan writes nothing");

        let after = apply(&fake, &change).await.unwrap();
        assert_eq!(after.sleep, Sleep::Never);
        assert_eq!(after.serial, Some(101), "the node after the restart");
        assert_eq!(fake.restarts(), 1);
        assert_eq!(fs::read_to_string(&change.path).unwrap(), change.text);
        assert!(rule_names(config.path(), &after));
        assert_eq!(rules_in(config.path()), vec![change.path.clone()]);
        let leftovers: Vec<_> = fs::read_dir(change.path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(leftovers.len(), 1, "no temporary file stays: {leftovers:?}");
    }

    #[tokio::test]
    async fn setup_leaves_other_files_alone_and_replaces_its_old_format() {
        let config = tempfile::tempdir().unwrap();
        let theirs = config
            .path()
            .join("wireplumber/wireplumber.conf.d/51-yeti-no-suspend.conf");
        fs::create_dir_all(theirs.parent().unwrap()).unwrap();
        fs::write(&theirs, "monitor.alsa.rules = []\n").unwrap();
        let old = Format::Lua.rule_path(config.path());
        fs::create_dir_all(old.parent().unwrap()).unwrap();
        fs::write(&old, "-- old\n").unwrap();

        let fake = Fake::new(vec![
            Ok(dump(vec![yeti(100, None), defaults(YETI)])),
            Ok(dump(vec![yeti(101, Some(serde_json::json!(0)))])),
        ]);
        let probe = probe(&fake, Some("default"), None).await;
        let Plan::Write(change) = plan(&fake, config.path(), probe).await.unwrap() else {
            panic!("a write");
        };
        assert_eq!(change.stale, vec![old.clone()]);
        apply(&fake, &change).await.unwrap();
        assert!(!old.exists(), "the Lua rule of Hush went with the update");
        assert_eq!(
            fs::read_to_string(&theirs).unwrap(),
            "monitor.alsa.rules = []\n",
            "a file of the user stays as it was"
        );
    }

    #[tokio::test]
    async fn a_node_that_stays_asleep_after_the_restart_is_an_error() {
        let config = tempfile::tempdir().unwrap();
        let fake = Fake::new(vec![
            Ok(dump(vec![yeti(100, None), defaults(YETI)])),
            Ok(dump(vec![yeti(101, None)])),
        ]);
        let probe = probe(&fake, Some("default"), None).await;
        let Plan::Write(change) = plan(&fake, config.path(), probe).await.unwrap() else {
            panic!("a write");
        };
        let error = apply(&fake, &change).await.unwrap_err().to_string();
        assert!(error.contains("still sleeps after 5 s"), "{error}");

        let gone = Fake::new(vec![Ok(dump(vec![yeti(100, None)]))]);
        let error = apply(&gone, &change).await.unwrap_err().to_string();
        assert!(error.contains("did not come back in 10 s"), "{error}");

        let mut broken = Fake::new(vec![Ok(dump(vec![yeti(100, None)]))]);
        broken.restart = Err("Unit wireplumber.service not found.".into());
        let error = format!("{:#}", apply(&broken, &change).await.unwrap_err());
        assert!(error.contains("did not restart"), "{error}");
        assert!(
            error.contains("Unit wireplumber.service not found."),
            "{error}"
        );
    }

    #[tokio::test]
    async fn setup_changes_nothing_for_an_awake_bluetooth_or_virtual_source() {
        let config = tempfile::tempdir().unwrap();
        let awake = Fake::new(vec![Ok(dump(vec![
            yeti(100, Some(serde_json::json!(0))),
            defaults(YETI),
        ]))]);
        let probe_awake = probe(&awake, Some("default"), None).await;
        assert!(matches!(
            plan(&awake, config.path(), probe_awake).await.unwrap(),
            Plan::Awake { by_hush: false, .. }
        ));

        let headset = node(
            7,
            70,
            "bluez_input.AC_80_0A_00_00_01.0",
            serde_json::json!({"device.api": "bluez5"}),
        );
        let fake = Fake::new(vec![Ok(dump(vec![
            headset,
            defaults("bluez_input.AC_80_0A_00_00_01.0"),
        ]))]);
        let probe_headset = probe(&fake, Some("default"), None).await;
        let error = plan(&fake, config.path(), probe_headset)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("Bluetooth"), "{error}");

        let virtual_mic = Fake::new(vec![Ok(dump(vec![node(
            9,
            90,
            "hush-e2e-mic-42",
            serde_json::json!({}),
        )]))]);
        let probe_virtual = probe(&virtual_mic, Some("default"), Some("hush-e2e-mic-42")).await;
        let error = plan(&virtual_mic, config.path(), probe_virtual)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("not an ALSA device"), "{error}");

        let mut no_wireplumber = Fake::new(vec![Ok(dump(vec![yeti(1, None), defaults(YETI)]))]);
        no_wireplumber.format = None;
        let probe_any = probe(&no_wireplumber, Some("default"), None).await;
        assert!(plan(&no_wireplumber, config.path(), probe_any)
            .await
            .is_err());

        let error = plan(&fake, config.path(), Probe::NoPipeWire)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("PipeWire does not answer"), "{error}");
        assert!(rules_in(config.path()).is_empty(), "no plan wrote a file");
        assert_eq!(awake.restarts() + fake.restarts(), 0);
    }

    #[tokio::test]
    async fn undo_removes_only_the_rule_of_hush() {
        let config = tempfile::tempdir().unwrap();
        let fake = Fake::new(vec![Ok(dump(vec![yeti(100, None), defaults(YETI)]))]);
        let nothing = undo(&fake, config.path(), Probe::NoPipeWire).await.unwrap();
        assert_eq!(nothing, Undone::Nothing);
        assert_eq!(fake.restarts(), 0, "no rule, no restart");

        let ours = Format::Conf.rule_path(config.path());
        fs::create_dir_all(ours.parent().unwrap()).unwrap();
        fs::write(
            &ours,
            Format::Conf.rule(&yeti_source(Sleep::Never)).unwrap(),
        )
        .unwrap();
        let theirs = ours.with_file_name("51-yeti-no-suspend.conf");
        fs::write(&theirs, "monitor.alsa.rules = []\n").unwrap();

        let fake = Fake::new(vec![
            Ok(dump(vec![
                yeti(100, Some(serde_json::json!(0))),
                defaults(YETI),
            ])),
            Ok(dump(vec![yeti(101, None)])),
        ]);
        let before = probe(&fake, Some("default"), None).await;
        assert!(rule_names(config.path(), &found(before.clone())));
        let undone = undo(&fake, config.path(), before).await.unwrap();
        let Undone::Removed { files, source } = undone else {
            panic!("the rule was there");
        };
        assert_eq!(files, vec![ours.clone()]);
        assert_eq!(source.unwrap().sleep, Sleep::After(5));
        assert!(!ours.exists());
        assert!(theirs.exists(), "a file of the user stays");
        assert_eq!(fake.restarts(), 1);
    }
}
