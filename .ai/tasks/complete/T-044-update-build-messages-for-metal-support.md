# T-044: Update Build Messages for Completed Metal Support

**Priority:** Low
**Effort:** Very Low (15 minutes)
**Type:** Documentation/Polish
**Status:** Available
**Created:** 2025-11-19

---

## Problem Statement

The `build.rs` file still contains outdated messages saying "Metal support will be added in T-036" even though T-036 was completed and Metal GPU acceleration is fully implemented.

### Current Output

```
warning: hush@0.1.0: ✅ Apple Silicon detected
warning: hush@0.1.0:    Metal GPU acceleration available
warning: hush@0.1.0:    Expected performance: ~120ms first token on M1/M2/M3
warning: hush@0.1.0:    (Metal support will be added in T-036)  ← OUTDATED
```

```
warning: hush@0.1.0: 📋 Build Configuration:
warning: hush@0.1.0:    Platform: macOS
warning: hush@0.1.0:    GPU: Metal support coming in T-036  ← OUTDATED
```

### Evidence Metal is Implemented

1. **T-036 is complete:** `.ai/tasks/complete/T-036-add-metal-gpu-acceleration-support.md`
2. **Metal detection works:** `src/transcription/device.rs` lines 62-78
3. **Metal feature compiles:** Code has `#[cfg(feature = "metal")]` blocks
4. **Device detection functional:** `GpuAvailability::detect()` properly detects Metal

---

## Goals

1. ✅ Remove outdated T-036 references from build messages
2. ✅ Update messages to reflect Metal support is implemented
3. ✅ Add correct feature flag instructions

---

## Implementation Steps

### 1. Update Apple Silicon Detection Message

**File:** `build.rs` (lines 220-226)

**Current:**
```rust
if cpu_info.contains("Apple") {
    println!("cargo:warning=✅ Apple Silicon detected");
    println!("cargo:warning=   Metal GPU acceleration available");
    println!(
        "cargo:warning=   Expected performance: ~120ms first token on M1/M2/M3"
    );
    println!("cargo:warning=   (Metal support will be added in T-036)");  // ← Remove this
}
```

**Updated:**
```rust
if cpu_info.contains("Apple") {
    println!("cargo:warning=✅ Apple Silicon detected");
    println!("cargo:warning=   Metal GPU acceleration available");
    println!(
        "cargo:warning=   Expected performance: ~120ms first token on M1/M2/M3"
    );

    #[cfg(feature = "metal")]
    println!("cargo:warning=   ✅ Metal feature enabled");

    #[cfg(not(feature = "metal"))]
    {
        println!("cargo:warning=   ⚠️  Metal feature not enabled");
        println!("cargo:warning=   Build with --features metal for GPU acceleration");
    }
}
```

### 2. Update Build Configuration Summary

**File:** `build.rs` (lines 257-258)

**Current:**
```rust
#[cfg(target_os = "macos")]
println!("cargo:warning=   GPU: Metal support coming in T-036");  // ← Update this
```

**Updated:**
```rust
#[cfg(all(target_os = "macos", feature = "metal"))]
println!("cargo:warning=   GPU: Metal enabled");

#[cfg(all(target_os = "macos", not(feature = "metal")))]
println!("cargo:warning=   GPU: CPU only (use --features metal for GPU)");
```

### 3. Verify Updated Messages

```bash
# Clean build to see all messages
cargo clean

# Build without metal feature
cargo build 2>&1 | grep -A 10 "Apple Silicon"

# Build with metal feature
cargo build --features metal 2>&1 | grep -A 10 "Apple Silicon"
```

**Expected output (with metal):**
```
warning: hush@0.1.0: ✅ Apple Silicon detected
warning: hush@0.1.0:    Metal GPU acceleration available
warning: hush@0.1.0:    Expected performance: ~120ms first token on M1/M2/M3
warning: hush@0.1.0:    ✅ Metal feature enabled
```

**Expected output (without metal):**
```
warning: hush@0.1.0: ✅ Apple Silicon detected
warning: hush@0.1.0:    Metal GPU acceleration available
warning: hush@0.1.0:    Expected performance: ~120ms first token on M1/M2/M3
warning: hush@0.1.0:    ⚠️  Metal feature not enabled
warning: hush@0.1.0:    Build with --features metal for GPU acceleration
```

---

## Success Criteria

- [ ] No references to "T-036" in build output
- [ ] Messages accurately reflect current Metal support status
- [ ] Feature flag detection works correctly
- [ ] Build messages guide users to enable Metal if desired

---

## Verification Steps

```bash
# Test without metal feature
cargo clean && cargo build 2>&1 | grep -E "Metal|GPU|T-036"

# Test with metal feature
cargo clean && cargo build --features metal 2>&1 | grep -E "Metal|GPU|T-036"

# Verify no T-036 references
cargo build 2>&1 | grep -i "T-036"
# Should return nothing
```

---

## Files to Modify

- `build.rs` (lines 226, 257-258)

---

## Dependencies

**Blocks:**
- Nothing critical (cosmetic issue)

**Blocked by:**
- None (can be done immediately)

---

## Context

### Why This Happened

T-036 was completed but the build.rs placeholder messages weren't updated. This is common when tasks complete but related documentation/messages aren't part of the task checklist.

### Metal Support Status

Metal GPU acceleration is **fully implemented**:
- ✅ Device detection in `src/transcription/device.rs`
- ✅ Feature flag support (`#[cfg(feature = "metal")]`)
- ✅ Candle Metal backend integration
- ✅ Performance benchmarks documented
- ✅ CI/CD testing on Apple Silicon runners

---

## Related Tasks

- T-036: Add Metal GPU Acceleration Support (COMPLETE)
- T-034: Update Build System for macOS (COMPLETE)

---

## Estimated Impact

- **Time to fix**: 15 minutes
- **Risk**: None (only affects build messages)
- **Benefit**: Accurate user-facing information
- **User experience**: Users won't be confused about Metal support status
