use crate::error::CaptureError;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

// ──────────────────────────────────────────────────────────────
// Streaming audio capture — channel-based alternative to record_to_wav.
//
//   Microphone ──▶ cpal callback ──▶ mono 16kHz f32
//        │
//        ├──▶ AudioChunk channel (for VAD, whisper, or any consumer)
//        └──▶ audio level (atomic, for UI meter)
//
// The existing record_to_wav blocks and writes to a file.
// AudioStream is non-blocking: consumers pull chunks via a
// crossbeam channel at their own pace. If the channel fills,
// oldest chunks are dropped (bounded channel) — consumers
// need fresh data, not stale audio.
//
// Mono-downmix + decimation resampling is shared with capture.rs
// via `resample::build_resampled_input_stream`.
//
// MultiAudioStream wraps two AudioStreams for multi-source capture,
// tagging each chunk with its source role for speaker attribution.
// ──────────────────────────────────────────────────────────────

/// Which logical source produced a chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceRole {
    /// The user's microphone (voice).
    Voice,
    /// System/call audio (remote participants).
    Call,
    /// Single source (no multi-source capture).
    Default,
}

/// A chunk of 16kHz mono f32 audio samples (~100ms each).
#[derive(Clone)]
pub struct AudioChunk {
    /// 16kHz mono f32 samples, typically 1600 samples (100ms).
    pub samples: Vec<f32>,
    /// RMS energy of this chunk (0.0–1.0 scale).
    pub rms: f32,
    /// Wall-clock timestamp when this chunk was captured.
    pub timestamp: Instant,
    /// Which source produced this chunk.
    pub source: SourceRole,
}

/// Shared audio level (0–100) for UI visualization.
/// Separate from capture.rs AUDIO_LEVEL to allow both APIs to coexist.
static STREAM_AUDIO_LEVEL: AtomicU32 = AtomicU32::new(0);

/// Get the current streaming audio input level (0–100).
pub fn stream_audio_level() -> u32 {
    STREAM_AUDIO_LEVEL.load(Ordering::Relaxed)
}

/// Handle to a running audio stream. Drop to stop capture.
pub struct AudioStream {
    _stream: cpal::Stream,
    stop: Arc<AtomicBool>,
    err_flag: Arc<AtomicBool>,
    /// Receive audio chunks from this channel.
    pub receiver: Receiver<AudioChunk>,
    /// The sample rate of output chunks (always 16000).
    pub sample_rate: u32,
    /// Name of the audio input device being used.
    pub device_name: String,
}

impl AudioStream {
    /// Start capturing from the specified (or default) input device.
    /// Returns a stream handle with a channel receiver for audio chunks.
    /// Chunks arrive at ~10Hz (100ms each at 16kHz = 1600 samples).
    pub fn start(device_override: Option<&str>) -> Result<Self, CaptureError> {
        let host = cpal::default_host();
        let device = crate::capture::select_input_device(&host, device_override)?;

        // Bounded channel: 64 chunks = ~6.4 seconds of buffered audio.
        let (tx, rx): (Sender<AudioChunk>, Receiver<AudioChunk>) = bounded(64);

        let stop = Arc::new(AtomicBool::new(false));
        let err_flag = Arc::new(AtomicBool::new(false));
        let chunk_size: usize = 1600; // 100ms at 16kHz

        let mut chunk_buf: Vec<f32> = Vec::with_capacity(chunk_size);

        let (stream, device_name, _config) = crate::resample::build_resampled_input_stream(
            &device,
            &stop,
            &err_flag,
            move |resampled: &[f32]| {
                for &sample in resampled {
                    chunk_buf.push(sample);

                    if chunk_buf.len() >= chunk_size {
                        let samples: Vec<f32> = chunk_buf.drain(..chunk_size).collect();
                        let rms = compute_rms(&samples);
                        let level = (rms * 2000.0).min(100.0) as u32;
                        STREAM_AUDIO_LEVEL.store(level, Ordering::Relaxed);
                        let _ = tx.try_send(AudioChunk {
                            samples,
                            rms,
                            timestamp: Instant::now(),
                            source: SourceRole::Default,
                        });
                    }
                }
            },
        )?;

        tracing::info!(device = %device_name, "streaming audio capture started");

        Ok(AudioStream {
            _stream: stream,
            stop,
            err_flag,
            receiver: rx,
            sample_rate: 16000,
            device_name,
        })
    }

    /// Returns true if the audio stream has encountered an error.
    pub fn has_error(&self) -> bool {
        self.err_flag.load(Ordering::Relaxed)
    }

    /// Stop the audio stream.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for AudioStream {
    fn drop(&mut self) {
        self.stop();
    }
}

fn compute_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
    (sum / samples.len() as f64).sqrt() as f32
}

/// Handle to two running audio streams (voice + call) for multi-source capture.
/// Produces tagged chunks from both sources on a single merged receiver.
pub struct MultiAudioStream {
    voice: AudioStream,
    call: AudioStream,
    _merge_thread: std::thread::JoinHandle<()>,
    stop: Arc<AtomicBool>,
    /// Receive tagged audio chunks from both sources.
    pub receiver: Receiver<AudioChunk>,
}

impl MultiAudioStream {
    /// Start capturing from two devices: one for voice (microphone) and one for
    /// call/system audio. Chunks from both sources arrive on a single receiver,
    /// tagged with their `SourceRole`.
    pub fn start(voice_device: Option<&str>, call_device: &str) -> Result<Self, CaptureError> {
        let voice = AudioStream::start(voice_device)?;
        let call = AudioStream::start(Some(call_device))?;

        let (tx, rx): (Sender<AudioChunk>, Receiver<AudioChunk>) = bounded(128);
        let stop = Arc::new(AtomicBool::new(false));

        let voice_rx = voice.receiver.clone();
        let call_rx = call.receiver.clone();
        let stop_clone = Arc::clone(&stop);
        let tx_clone = tx.clone();

        let merge_thread = std::thread::spawn(move || {
            let timeout = std::time::Duration::from_millis(50);
            while !stop_clone.load(Ordering::Relaxed) {
                // Drain voice chunks
                while let Ok(mut chunk) = voice_rx.try_recv() {
                    chunk.source = SourceRole::Voice;
                    let _ = tx.try_send(chunk);
                }
                // Drain call chunks
                while let Ok(mut chunk) = call_rx.try_recv() {
                    chunk.source = SourceRole::Call;
                    let _ = tx_clone.try_send(chunk);
                }
                std::thread::sleep(timeout);
            }
        });

        tracing::info!(
            voice = %voice.device_name,
            call = %call.device_name,
            "multi-source audio capture started"
        );

        Ok(MultiAudioStream {
            voice,
            call,
            _merge_thread: merge_thread,
            stop,
            receiver: rx,
        })
    }

    /// Returns true if either audio stream has encountered an error.
    pub fn has_error(&self) -> bool {
        self.voice.has_error() || self.call.has_error()
    }

    /// Name of the voice (microphone) device.
    pub fn voice_device_name(&self) -> &str {
        &self.voice.device_name
    }

    /// Name of the call (system audio) device.
    pub fn call_device_name(&self) -> &str {
        &self.call.device_name
    }
}

impl Drop for MultiAudioStream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.voice.stop();
        self.call.stop();
    }
}
