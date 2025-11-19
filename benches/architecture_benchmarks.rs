/// Performance benchmarks for trait-based architecture
///
/// These benchmarks compare the performance overhead of trait objects
/// vs concrete types to ensure the refactoring doesn't introduce
/// significant performance regression.
///
/// Run with: cargo bench
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use hush::adapters::CpalAudioAdapter;
use hush::core::mocks::{MockAudioSource, MockTranscriber};
use hush::core::traits::{AudioBuffer, AudioSource, Transcriber};
use std::time::Duration;

/// Benchmark: Method call overhead (trait object vs direct)
fn benchmark_method_call_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("method_call_overhead");

    // Direct method call (baseline)
    let mut mock = MockAudioSource::new();
    group.bench_function("direct_is_recording", |b| {
        b.iter(|| black_box(mock.is_recording()))
    });

    // Trait object method call
    let mock_trait: Box<dyn AudioSource> = Box::new(MockAudioSource::new());
    group.bench_function("trait_is_recording", |b| {
        b.iter(|| black_box(mock_trait.is_recording()))
    });

    group.finish();
}

/// Benchmark: Recording lifecycle
fn benchmark_recording_lifecycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("recording_lifecycle");

    group.bench_function("mock_start_stop", |b| {
        b.iter(|| {
            let mut audio = MockAudioSource::new();
            audio.start_recording().unwrap();
            let _ = black_box(audio.stop_recording().unwrap());
        })
    });

    group.finish();
}

/// Benchmark: Transcription with mock
fn benchmark_transcription(c: &mut Criterion) {
    let mut group = c.benchmark_group("transcription");

    // Setup
    let rt = tokio::runtime::Runtime::new().unwrap();
    let transcriber = MockTranscriber::new();
    let audio = AudioBuffer::new(vec![0.1f32; 16000], 16000, 1);

    group.bench_function("mock_transcribe", |b| {
        b.to_async(&rt).iter(|| async {
            let result = transcriber.transcribe(&audio).await.unwrap();
            black_box(result)
        })
    });

    group.finish();
}

/// Benchmark: Component creation
fn benchmark_component_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("component_creation");

    group.bench_function("mock_audio_source", |b| {
        b.iter(|| black_box(MockAudioSource::new()))
    });

    group.bench_function("mock_transcriber", |b| {
        b.iter(|| black_box(MockTranscriber::new()))
    });

    group.finish();
}

/// Benchmark: State checks
fn benchmark_state_checks(c: &mut Criterion) {
    use hush::core::state::{AppState, StateMachine};

    let mut group = c.benchmark_group("state_checks");

    let state = StateMachine::new();

    group.bench_function("current_state", |b| b.iter(|| black_box(state.current())));

    group.bench_function("is_recording", |b| {
        b.iter(|| black_box(state.current().is_recording()))
    });

    group.finish();
}

/// Benchmark: Builder pattern
fn benchmark_builder(c: &mut Criterion) {
    use hush::application::HushAppBuilder;

    let mut group = c.benchmark_group("builder");

    group.bench_function("app_construction", |b| {
        b.iter(|| {
            let app = HushAppBuilder::new()
                .with_audio(Box::new(MockAudioSource::new()))
                .with_transcriber(Box::new(MockTranscriber::new()))
                .with_text_output(Box::new(hush::core::mocks::MockTextOutput::new()))
                .with_input_trigger(Box::new(hush::core::mocks::MockInputTrigger::new()))
                .manual_mode()
                .build()
                .unwrap();
            black_box(app)
        })
    });

    group.finish();
}

/// Benchmark: Full pipeline (mocked)
fn benchmark_full_pipeline(c: &mut Criterion) {
    use hush::application::HushAppBuilder;

    let mut group = c.benchmark_group("full_pipeline");
    group.measurement_time(Duration::from_secs(10));

    let rt = tokio::runtime::Runtime::new().unwrap();

    group.bench_function("record_transcribe_insert", |b| {
        b.to_async(&rt).iter(|| async {
            let mut app = HushAppBuilder::new()
                .with_audio(Box::new(MockAudioSource::new()))
                .with_transcriber(Box::new(MockTranscriber::new()))
                .with_text_output(Box::new(hush::core::mocks::MockTextOutput::new()))
                .with_input_trigger(Box::new(hush::core::mocks::MockInputTrigger::new()))
                .manual_mode()
                .with_notifications(false)
                .build()
                .unwrap();

            app.handle_recording_start().await.unwrap();
            app.handle_recording_stop().await.unwrap();
            black_box(app)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_method_call_overhead,
    benchmark_recording_lifecycle,
    benchmark_transcription,
    benchmark_component_creation,
    benchmark_state_checks,
    benchmark_builder,
    benchmark_full_pipeline,
);

criterion_main!(benches);
