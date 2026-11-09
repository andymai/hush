//! The CPU floor of a prebuilt binary.
//!
//! Release builds compile ggml for x86-64-v3 and set `HUSH_CPU_FLOOR` at
//! build time. On an older CPU, the first call into ggml stops the process
//! with SIGILL, so Hush checks the CPU first and names the fix. A build from
//! source has no floor, because ggml targets the CPU that built it.

use crate::Result;

pub const X86_64_V3: &str = "x86-64-v3";

/// ggml's x86-64-v3 build also enables SSE4.2, which every AVX2 CPU has.
pub const X86_64_V3_FEATURES: &[&str] = &["avx", "avx2", "fma", "f16c", "bmi2"];

/// The fix that `hush doctor` prints for an unsupported CPU.
pub const FIX: &str =
    "build from source with `make release`, or set backend = \"groq\" or \"openai\" under [transcription]";

/// The floor this binary was built for, or `None` for a build from source.
pub fn floor() -> Option<&'static str> {
    option_env!("HUSH_CPU_FLOOR")
}

fn required_features() -> &'static [&'static str] {
    match floor() {
        Some(X86_64_V3) => X86_64_V3_FEATURES,
        _ => &[],
    }
}

/// The features of `required` that are not in `present`.
pub fn missing_features<'a>(required: &[&'a str], present: &[&str]) -> Vec<&'a str> {
    required
        .iter()
        .copied()
        .filter(|feature| !present.contains(feature))
        .collect()
}

fn detected_features() -> Vec<&'static str> {
    X86_64_V3_FEATURES
        .iter()
        .copied()
        .filter(|feature| detected(feature))
        .collect()
}

#[cfg(target_arch = "x86_64")]
fn detected(feature: &str) -> bool {
    match feature {
        "avx" => std::arch::is_x86_feature_detected!("avx"),
        "avx2" => std::arch::is_x86_feature_detected!("avx2"),
        "fma" => std::arch::is_x86_feature_detected!("fma"),
        "f16c" => std::arch::is_x86_feature_detected!("f16c"),
        "bmi2" => std::arch::is_x86_feature_detected!("bmi2"),
        _ => false,
    }
}

#[cfg(not(target_arch = "x86_64"))]
fn detected(_feature: &str) -> bool {
    false
}

/// The features that ggml in this binary needs and this CPU does not have.
pub fn missing() -> Vec<&'static str> {
    missing_features(required_features(), &detected_features())
}

pub fn supported() -> bool {
    missing().is_empty()
}

/// Call before anything that calls into ggml, the backend registry included.
pub fn ensure_supported() -> Result<()> {
    let missing = missing();
    if missing.is_empty() {
        return Ok(());
    }
    Err(anyhow::anyhow!(unsupported_message(
        floor().unwrap_or(X86_64_V3),
        &missing
    )))
}

/// The features in upper case, as CPU vendors write them.
pub fn feature_list(features: &[&str]) -> String {
    features
        .iter()
        .map(|feature| feature.to_ascii_uppercase())
        .collect::<Vec<_>>()
        .join(", ")
}

fn unsupported_message(floor: &str, missing: &[&str]) -> String {
    format!(
        "This CPU does not have {}. This prebuilt Hush binary needs an {} CPU \
         (Intel Haswell from 2013 or newer, AMD Excavator or Zen or newer) to run Whisper on this machine.\n\
         To fix this, do one of these:\n  \
         - Build Hush from source with `make release`. That build targets this CPU. See INSTALL.md.\n  \
         - Use a cloud backend: set backend = \"groq\" or \"openai\" under [transcription].",
        feature_list(missing),
        floor
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cpu_with_every_feature_misses_nothing() {
        assert!(missing_features(X86_64_V3_FEATURES, X86_64_V3_FEATURES).is_empty());
        let zen4 = ["sse4.2", "avx", "avx2", "fma", "f16c", "bmi2", "avx512f"];
        assert!(missing_features(X86_64_V3_FEATURES, &zen4).is_empty());
    }

    #[test]
    fn an_older_cpu_misses_the_features_it_does_not_have() {
        let ivy_bridge = ["avx", "f16c"];
        assert_eq!(
            missing_features(X86_64_V3_FEATURES, &ivy_bridge),
            ["avx2", "fma", "bmi2"]
        );
        assert_eq!(
            missing_features(X86_64_V3_FEATURES, &[]),
            X86_64_V3_FEATURES
        );
    }

    #[test]
    fn a_build_from_source_runs_on_any_cpu() {
        assert!(missing_features(&[], &[]).is_empty());
        if floor().is_none() {
            assert!(supported());
            assert!(ensure_supported().is_ok());
        }
    }

    #[test]
    fn the_message_names_the_missing_features_and_both_fixes() {
        let message = unsupported_message(X86_64_V3, &["avx2", "fma"]);
        assert!(message.contains("does not have AVX2, FMA."), "{message}");
        assert!(message.contains("an x86-64-v3 CPU"), "{message}");
        assert!(message.contains("make release"), "{message}");
        assert!(message.contains("backend = \"groq\""), "{message}");
        assert!(!message.contains("F16C"), "{message}");
    }
}
