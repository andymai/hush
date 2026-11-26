//! The hardware tune's arithmetic: which model to recommend from measured
//! speeds, and how close a transcript comes to the words that were read.
//! `hush tune` measures one model on each device; the speeds of the larger
//! models are estimates until a model is measured after its download.

use super::gpu_fallback::GpuFingerprint;
use super::models::{catalogue_entry, ModelSize};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The models the tune picks from, least accurate first. Parakeet stays
/// out: it covers 25 languages and reads no prompt.
pub const LADDER: [ModelSize; 6] = [
    ModelSize::Tiny,
    ModelSize::Base,
    ModelSize::Small,
    ModelSize::Medium,
    ModelSize::LargeV3Turbo,
    ModelSize::LargeV3,
];

/// How much faster than real time a model must run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Preference {
    /// The text within about half a second after a 10 s take
    Faster,
    /// The text within about one second after a 10 s take
    #[default]
    Balanced,
    /// The text within about two seconds after a 10 s take
    Accurate,
}

impl Preference {
    /// Seconds of audio per second of work that the pick must reach.
    pub fn min_speed(self) -> f32 {
        match self {
            Preference::Faster => 20.0,
            Preference::Balanced => 10.0,
            Preference::Accurate => 5.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    Gpu,
    Cpu,
}

/// One model timed on one device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Measured {
    pub device: DeviceKind,
    pub model: ModelSize,
    /// Seconds of audio per second of work
    pub speed: f32,
    /// The device's memory, when ggml reports it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_total: Option<u64>,
    /// Word error rate against the words that were read, when known
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub word_error_rate: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pick {
    pub model: ModelSize,
    pub device: DeviceKind,
    /// The estimated or measured speed
    pub speed: f32,
}

/// How many times longer than base a model takes on a device. Turbo's
/// numbers are measured (RTX 4080 SUPER through Vulkan: 2.2; Ryzen 9 7950X3D:
/// 12, where its 32-layer encoder dominates); the others are estimates from
/// model size and depth on the same scale.
fn relative_cost(model: ModelSize, device: DeviceKind) -> f32 {
    use ModelSize::*;
    match (device, model) {
        (_, Base) => 1.0,
        (DeviceKind::Gpu, Tiny) => 0.6,
        (DeviceKind::Gpu, Small) => 1.8,
        (DeviceKind::Gpu, Medium) => 3.5,
        (DeviceKind::Gpu, LargeV3Turbo) => 2.2,
        (DeviceKind::Gpu, _) => 5.0,
        (DeviceKind::Cpu, Tiny) => 0.45,
        (DeviceKind::Cpu, Small) => 3.5,
        (DeviceKind::Cpu, Medium) => 10.0,
        (DeviceKind::Cpu, LargeV3Turbo) => 12.0,
        (DeviceKind::Cpu, _) => 24.0,
    }
}

/// An estimate is this much more careful than the cost table.
const MARGIN: f32 = 1.3;

/// The speed of `model` on the device of `measured`. A measured model keeps
/// its measured speed.
pub fn estimate(measured: &Measured, model: ModelSize) -> f32 {
    if model == measured.model {
        return measured.speed;
    }
    let base = measured.speed * relative_cost(measured.model, measured.device);
    base / (relative_cost(model, measured.device) * MARGIN)
}

/// A model fits when the device has twice its file size.
fn fits(model: ModelSize, memory_total: Option<u64>) -> bool {
    let size = catalogue_entry(model).map_or(0, |info| info.expected_size);
    memory_total.is_none_or(|total| size.saturating_mul(2) <= total)
}

/// The most accurate model that reaches the preference's speed on a measured
/// device, the GPU first when both tie. Without one, the fastest small model.
pub fn pick(measured: &[Measured], preference: Preference) -> Option<Pick> {
    let min = preference.min_speed();
    let mut best: Option<(usize, Pick)> = None;
    for run in measured {
        for (rank, model) in LADDER.iter().enumerate() {
            let speed = estimate(run, *model);
            if speed < min || !fits(*model, run.memory_total) {
                continue;
            }
            let better = match &best {
                None => true,
                Some((best_rank, best_pick)) => {
                    rank > *best_rank
                        || (rank == *best_rank
                            && run.device == DeviceKind::Gpu
                            && best_pick.device == DeviceKind::Cpu)
                },
            };
            if better {
                best = Some((
                    rank,
                    Pick {
                        model: *model,
                        device: run.device,
                        speed,
                    },
                ));
            }
        }
    }
    best.map(|(_, pick)| pick).or_else(|| {
        let fastest = measured.iter().max_by(|a, b| a.speed.total_cmp(&b.speed))?;
        Some(Pick {
            model: ModelSize::Tiny,
            device: fastest.device,
            speed: estimate(fastest, ModelSize::Tiny),
        })
    })
}

/// The share of the reference's words that the hypothesis gets wrong, after
/// case and punctuation are dropped: substitutions, insertions, and deletions
/// over the number of reference words.
pub fn word_error_rate(reference: &str, hypothesis: &str) -> f32 {
    let words = |text: &str| -> Vec<String> {
        text.split_whitespace()
            .map(|word| {
                word.chars()
                    .filter(|c| c.is_alphanumeric() || *c == '\'')
                    .flat_map(char::to_lowercase)
                    .collect::<String>()
            })
            .filter(|word| !word.is_empty())
            .collect()
    };
    let (reference, hypothesis) = (words(reference), words(hypothesis));
    if reference.is_empty() {
        return if hypothesis.is_empty() { 0.0 } else { 1.0 };
    }
    let mut row: Vec<usize> = (0..=hypothesis.len()).collect();
    for (i, said) in reference.iter().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, heard) in hypothesis.iter().enumerate() {
            let kept = row[j + 1];
            row[j + 1] = if said == heard {
                previous
            } else {
                1 + previous.min(row[j]).min(row[j + 1])
            };
            previous = kept;
        }
    }
    row[hypothesis.len()] as f32 / reference.len() as f32
}

/// What the last tune measured and picked, for the Speech page and for the
/// recommendation of the setup wizard and the doctor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TuneResult {
    /// The GPU at the time; a new GPU or driver makes the result stale
    pub gpu: Option<GpuFingerprint>,
    pub preference: Preference,
    pub measured: Vec<Measured>,
    pub pick: Pick,
}

impl TuneResult {
    pub fn path() -> PathBuf {
        crate::config::paths::state_dir().join("tune.json")
    }

    pub fn load_from(path: &Path) -> Option<Self> {
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
    }

    pub fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    /// The pick still holds for `gpu`, the GPU that this start found.
    pub fn holds_for(&self, gpu: Option<&GpuFingerprint>) -> bool {
        self.gpu.as_ref() == gpu
    }

    /// The saved tune, when it timed the GPU and driver that Hush uses now.
    pub fn current() -> Option<Self> {
        let gpu = crate::transcription::device::GpuAvailability::detect();
        let fingerprint = gpu.available.then(|| GpuFingerprint::current(gpu));
        Self::load_from(&Self::path()).filter(|result| result.holds_for(fingerprint.as_ref()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(device: DeviceKind, speed: f32, memory_total: Option<u64>) -> Measured {
        Measured {
            device,
            model: ModelSize::Base,
            speed,
            memory_total,
            word_error_rate: None,
        }
    }

    #[test]
    fn a_fast_gpu_gets_the_most_accurate_model_that_keeps_up() {
        let measured = [
            run(DeviceKind::Gpu, 120.0, Some(16 << 30)),
            run(DeviceKind::Cpu, 6.0, None),
        ];
        let pick = pick(&measured, Preference::Balanced).unwrap();
        assert_eq!(pick.device, DeviceKind::Gpu);
        assert_eq!(pick.model, ModelSize::LargeV3);
        let faster = super::pick(&measured, Preference::Faster).unwrap();
        assert!(
            LADDER.iter().position(|m| *m == faster.model)
                < LADDER.iter().position(|m| *m == pick.model)
        );
    }

    #[test]
    fn a_cpu_alone_gets_a_smaller_model() {
        let pick = pick(&[run(DeviceKind::Cpu, 30.0, None)], Preference::Balanced).unwrap();
        assert_eq!(pick.device, DeviceKind::Cpu);
        assert_eq!(
            pick.model,
            ModelSize::Base,
            "small at 30 / (3.5 * 1.3) = 6.6 misses the budget"
        );
    }

    #[test]
    fn a_small_gpu_memory_rules_out_the_large_models() {
        let measured = [run(DeviceKind::Gpu, 500.0, Some(2 << 30))];
        let pick = pick(&measured, Preference::Accurate).unwrap();
        assert!(!matches!(
            pick.model,
            ModelSize::LargeV3 | ModelSize::Medium
        ));
    }

    #[test]
    fn a_slow_machine_still_gets_tiny() {
        let pick = pick(&[run(DeviceKind::Cpu, 1.5, None)], Preference::Balanced).unwrap();
        assert_eq!(pick.model, ModelSize::Tiny);
        assert!(pick.speed < Preference::Balanced.min_speed());
        assert_eq!(super::pick(&[], Preference::Balanced), None);
    }

    #[test]
    fn a_measured_model_keeps_its_speed() {
        let measured = run(DeviceKind::Gpu, 40.0, None);
        assert_eq!(estimate(&measured, ModelSize::Base), 40.0);
        assert!(estimate(&measured, ModelSize::Medium) < 40.0 / 3.5);
    }

    #[test]
    fn the_word_error_rate_ignores_case_and_punctuation() {
        let reference = "And so, my fellow Americans: ask not what your country can do for you.";
        assert_eq!(
            word_error_rate(
                reference,
                "and so my fellow americans ask not what your country can do for you"
            ),
            0.0
        );
        let one_wrong = word_error_rate(
            reference,
            "And so my fellow Americans, ask not what your county can do for you.",
        );
        assert!((one_wrong - 1.0 / 14.0).abs() < 1e-6, "{one_wrong}");
        assert_eq!(word_error_rate(reference, ""), 1.0);
        assert_eq!(word_error_rate("", ""), 0.0);
    }

    #[test]
    fn a_result_holds_only_for_its_gpu() {
        let gpu = GpuFingerprint {
            gpu: "NVIDIA GeForce RTX 4080 SUPER".into(),
            backend: "Vulkan".into(),
            driver: "NVIDIA 620.73.04".into(),
        };
        let result = TuneResult {
            gpu: Some(gpu.clone()),
            preference: Preference::Balanced,
            measured: vec![run(DeviceKind::Gpu, 100.0, None)],
            pick: Pick {
                model: ModelSize::LargeV3Turbo,
                device: DeviceKind::Gpu,
                speed: 25.0,
            },
        };
        assert!(result.holds_for(Some(&gpu)));
        assert!(!result.holds_for(None));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state/tune.json");
        result.save_to(&path).unwrap();
        assert_eq!(TuneResult::load_from(&path), Some(result));
    }

    #[test]
    fn a_measured_larger_model_anchors_the_estimates() {
        let turbo = Measured {
            device: DeviceKind::Gpu,
            model: ModelSize::LargeV3Turbo,
            speed: 128.6,
            memory_total: None,
            word_error_rate: Some(0.0),
        };
        assert_eq!(estimate(&turbo, ModelSize::LargeV3Turbo), 128.6);
        let base = estimate(&turbo, ModelSize::Base);
        assert!((base - 128.6 * 2.2 / 1.3).abs() < 0.01, "{base}");
    }
}
