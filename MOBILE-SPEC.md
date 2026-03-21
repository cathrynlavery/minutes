# Minutes Mobile — Technical Spec

> On-device transcription, fully offline, same pipeline.

**Status**: Exploration / RFC
**Date**: 2026-03-21
**Author**: Claude (exploration for Mat Silverstein)

---

## Goals

1. Record + transcribe on phone — no Mac required
2. Same markdown output format (YAML frontmatter + transcript)
3. Fully offline capable (on-device Whisper)
4. Sync meetings to desktop for search/MCP/Claude access
5. Timestamped notes during recording (mobile `minutes note`)

## Non-Goals (v1)

- Speaker diarization (requires pyannote / significant mobile work)
- LLM summarization on-device (use cloud or defer to desktop)
- Screen context capture
- Folder watcher
- Desktop app feature parity

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Minutes Mobile (iOS first, Android follow)                 │
│                                                             │
│  ┌──────────┐   ┌──────────────┐   ┌──────────────────┐   │
│  │ Capture   │──▶│ Transcribe   │──▶│ Markdown Write   │   │
│  │           │   │              │   │                  │   │
│  │ AVAudio   │   │ whisper.cpp  │   │ YAML frontmatter │   │
│  │ Engine    │   │ (Core ML)    │   │ + transcript     │   │
│  │ (native)  │   │ tiny/base    │   │                  │   │
│  └──────────┘   └──────────────┘   └────────┬─────────┘   │
│                                              │              │
│  ┌──────────┐                     ┌─────────▼──────────┐  │
│  │ Notes UI  │────────────────────▶│ Local Storage      │  │
│  │ (tap to   │                     │ Documents/meetings/ │  │
│  │  annotate)│                     └─────────┬──────────┘  │
│  └──────────┘                               │              │
│                                    ┌────────▼──────────┐   │
│                                    │ Sync (iCloud /    │   │
│                                    │ manual export)    │   │
│                                    └───────────────────┘   │
└─────────────────────────────────────────────────────────────┘
         │
         │  iCloud Drive / manual sync
         ▼
┌─────────────────────────────────────────────────────────────┐
│  Desktop (existing)                                         │
│  ~/meetings/*.md  ←  same format, searchable via MCP        │
│  Optional: re-process with diarization + summarization      │
└─────────────────────────────────────────────────────────────┘
```

---

## Crate Strategy

### What exists today

```
minutes/crates/
├── core/       # Full engine: capture + transcribe + diarize + summarize + pipeline
├── reader/     # Read-only meeting parser (no audio deps) — already mobile-ready
├── cli/        # CLI binary
└── mcp/        # MCP server (TypeScript)
```

### Proposed additions

```
minutes/crates/
├── core/       # (unchanged) Full desktop engine
├── reader/     # (unchanged) Already mobile-ready — zero audio deps
├── mobile/     # NEW — mobile-specific capture + transcription + pipeline
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs          # Public API: record(), transcribe(), process()
│       ├── transcribe.rs   # whisper.cpp via C FFI (not whisper-rs)
│       └── pipeline.rs     # Simplified: capture → transcribe → markdown write
├── cli/
└── mcp/
```

**Key insight**: Don't port `minutes-core` to mobile. Create a thin `minutes-mobile` crate that reuses `minutes-reader` for types/parsing and reimplements only capture + transcribe for mobile targets.

### Shared code (via `minutes-reader`)

The `reader` crate already exists as a dependency-free meeting parser:
- `Frontmatter`, `ActionItem`, `Decision` structs
- `parse_meeting()`, `split_frontmatter()`
- `search()` for walking meeting directories
- **Zero audio dependencies** — compiles for any target

### What `minutes-mobile` provides

| Function | Signature | Notes |
|----------|-----------|-------|
| `transcribe()` | `fn transcribe(audio_path: &Path, model_path: &Path) -> Result<String>` | whisper.cpp via C FFI |
| `process()` | `fn process(audio_path: &Path, config: &MobileConfig) -> Result<WriteResult>` | Simplified pipeline |

---

## Whisper on Mobile — Implementation

### iOS (primary target)

**Option A — whisper.cpp via XCFramework (recommended)**
- whisper.cpp ships official iOS examples (`whisper.objc`, `whisper.swiftui`)
- Build as XCFramework, link from Rust via `cc` crate or call from Swift shell
- Core ML backend for Apple Neural Engine acceleration (3x faster than CPU)
- Model: `ggml-tiny.bin` (75MB) or `ggml-base.bin` (142MB)

**Option B — WhisperKit (Swift-native)**
- Argmax's WhisperKit: optimized for Apple Silicon, includes diarization
- Pure Swift, can't call from Rust directly — would need Swift app shell
- Benefit: built-in streaming, VAD, diarization

**Option C — whisper.cpp via Rust FFI**
- Compile whisper.cpp for `aarch64-apple-ios`
- Call via `whisper-rs` or raw FFI bindings
- Same Rust code as desktop, different compile target
- Risk: whisper-rs may need patches for iOS (no upstream iOS CI)

**Recommendation**: Option A for v1 (proven path, official examples exist), with the Rust pipeline calling whisper.cpp via C FFI. The app shell is Swift, the brain is Rust.

### Android (follow-on)

- whisper.cpp compiles for Android via NDK
- JNI bindings exist in the whisper.cpp repo
- Or: Rust FFI via `jni` crate (Rust → JNI → whisper.cpp)
- Model quantization (Q5/Q8) keeps memory under 100MB

### Model Management

```
Models bundled or downloaded on first launch:

tiny (75 MB)   — fastest, good enough for memos and short recordings
base (142 MB)  — better accuracy, still fast on modern phones
small (466 MB) — desktop-quality, feasible on phones with 6GB+ RAM

Default: tiny (ship in app bundle or download on first launch)
Storage: App Documents/models/ggml-tiny.bin
```

### Performance Expectations

| Model | Device | 30s audio | 5min audio | Memory |
|-------|--------|-----------|------------|--------|
| tiny | iPhone 13+ (ANE) | ~1s | ~10s | ~125MB |
| tiny | iPhone 13+ (CPU) | ~3s | ~30s | ~125MB |
| base | iPhone 13+ (ANE) | ~2s | ~20s | ~200MB |
| tiny | Pixel 7+ | ~2s | ~20s | ~125MB |

*Based on published whisper.cpp mobile benchmarks. ANE = Apple Neural Engine via Core ML.*

---

## Audio Capture — Mobile

### Current desktop approach (cpal)

```rust
// crates/core/src/capture.rs
pub fn record_to_wav(
    output_path: &Path,
    stop_flag: Arc<AtomicBool>,
    config: &Config,
) -> Result<(), CaptureError>
```

cpal handles audio I/O cross-platform but has limited iOS/Android support.

### Mobile approach

**iOS**: AVAudioEngine (Swift)
```swift
// Native Swift capture → WAV file → pass to Rust for transcription
let engine = AVAudioEngine()
let inputNode = engine.inputNode
let format = AVAudioFormat(commonFormat: .pcmFormatFloat32,
                           sampleRate: 16000, channels: 1,
                           interleaved: false)!
inputNode.installTap(onBus: 0, bufferSize: 4096, format: format) { buffer, time in
    // Write PCM samples to WAV file
}
engine.prepare()
try engine.start()
```

**Android**: AudioRecord (Kotlin) or Oboe (C++)
```kotlin
val recorder = AudioRecord(
    MediaRecorder.AudioSource.MIC,
    16000, // 16kHz
    AudioFormat.CHANNEL_IN_MONO,
    AudioFormat.ENCODING_PCM_16BIT,
    bufferSize
)
recorder.startRecording()
```

**Interface contract** — mobile capture produces the same output as desktop:
- 16kHz mono 16-bit PCM WAV file
- Same path-based handoff to transcription

---

## App Structure (iOS)

```
MinutesApp/
├── MinutesApp.xcodeproj
├── Sources/
│   ├── App/
│   │   ├── MinutesApp.swift           # App entry point
│   │   └── ContentView.swift          # Main UI
│   ├── Recording/
│   │   ├── RecordingView.swift        # Record button, timer, waveform
│   │   ├── AudioCapture.swift         # AVAudioEngine wrapper → WAV
│   │   └── NotesView.swift            # Timestamped notes during recording
│   ├── Meetings/
│   │   ├── MeetingListView.swift      # Browse past meetings
│   │   └── MeetingDetailView.swift    # Read transcript + summary
│   ├── Processing/
│   │   ├── WhisperBridge.swift        # Swift ↔ whisper.cpp C API
│   │   └── Pipeline.swift            # capture → transcribe → save
│   └── Sync/
│       └── iCloudSync.swift           # Sync ~/meetings/ via iCloud Drive
├── Rust/                              # Rust library (markdown, reader, config)
│   └── minutes-mobile/               # Compiled as .a static lib
└── Models/
    └── ggml-tiny.bin                  # Bundled or downloaded
```

### UI Screens

**1. Record (primary)**
- Large record button (tap to start/stop)
- Live audio waveform / level meter
- Timer showing elapsed time
- "Add Note" button → text field for timestamped annotation
- Processing indicator after stop (transcribing...)

**2. Meetings List**
- Chronological list of processed meetings
- Title, date, duration, word count
- Search bar (full-text via `minutes-reader`)
- Pull-to-refresh for synced meetings from desktop

**3. Meeting Detail**
- Rendered markdown (transcript + summary if available)
- Action items highlighted
- Share button (export .md file)
- "Process on Desktop" button (marks for re-processing with diarization/summarization)

**4. Settings**
- Model selection (tiny / base / small)
- Model download manager
- iCloud sync toggle
- Output directory

---

## Sync Strategy

### Option A — iCloud Drive (recommended for iOS)

```
iCloud Drive/
└── Minutes/
    └── meetings/
        ├── 2026-03-21-morning-standup.md    # Created on phone
        ├── 2026-03-20-advisor-call.md       # Created on desktop
        └── memos/
            └── 2026-03-21-idea-about-pricing.md  # Voice memo on phone
```

- Same directory structure as desktop `~/meetings/`
- Desktop `minutes watch` can monitor iCloud Drive path for re-processing
- Bidirectional: phone-created meetings appear on desktop, desktop meetings readable on phone
- Config: `output_dir = "~/Library/Mobile Documents/com~apple~CloudDocs/Minutes/meetings"`

### Option B — Manual export

- Share sheet: export `.md` file via AirDrop, Files, email
- Import: open `.md` files from Files app
- No automatic sync, user-controlled

### Desktop re-processing flow

```
Phone creates:     ~/meetings/2026-03-21-standup.md  (transcript only, no summary)
Desktop detects:   New file in watched directory
Desktop runs:      Diarization + summarization (optional, config-driven)
Desktop updates:   Same .md file with added summary, speaker labels, action items
Phone sees:        Updated file via iCloud sync
```

This leverages the existing pipeline — `minutes watch` already processes new audio files. We'd add a mode to re-process markdown files that lack summaries.

---

## Simplified Mobile Pipeline

```rust
// crates/mobile/src/pipeline.rs

pub struct MobileConfig {
    pub model_path: PathBuf,      // path to ggml-tiny.bin
    pub output_dir: PathBuf,      // Documents/meetings/
    pub content_type: ContentType, // Meeting or Memo
}

pub fn process(
    audio_path: &Path,
    notes: Option<&str>,       // user's timestamped notes
    config: &MobileConfig,
) -> Result<WriteResult> {
    // 1. Transcribe (mandatory)
    let transcript = transcribe(audio_path, &config.model_path)?;

    // 2. Check minimum words
    let word_count = transcript.split_whitespace().count();
    if word_count < 5 {
        return Err(MobileError::NoSpeech);
    }

    // 3. Build frontmatter
    let frontmatter = Frontmatter {
        title: auto_title(&transcript, config.content_type),
        date: chrono::Local::now(),
        duration: audio_duration(audio_path),
        word_count,
        content_type: config.content_type,
        // No: attendees, diarization, summary, action_items (desktop adds these)
        ..Default::default()
    };

    // 4. Write markdown
    let result = markdown::write(frontmatter, &transcript, None, notes, config)?;

    // 5. Log event (non-fatal)
    let _ = events::append_event(MinutesEvent::AudioProcessed { ... });

    Ok(result)
}
```

**What's skipped vs desktop:**
| Step | Desktop | Mobile v1 |
|------|---------|-----------|
| Capture | cpal | Native (AVAudioEngine / AudioRecord) |
| Transcribe | whisper-rs | whisper.cpp C FFI |
| Diarize | pyannote (Python) | Skipped |
| Summarize | LLM (cloud/local) | Skipped (or cloud-optional) |
| Notes | File-based | In-app UI |
| Markdown write | Same | Same (via reader types) |
| Screen context | screencapture | Skipped |
| Event log | JSONL | Same |

---

## Implementation Phases

### Phase 1 — Proof of Concept (1-2 weeks)

**Goal**: whisper.cpp transcribing audio on an iPhone, output is a .md file.

- [ ] Compile whisper.cpp for iOS (XCFramework)
- [ ] Minimal Swift app: record button → WAV file → whisper transcribe → show text
- [ ] Verify tiny model performance on target devices (iPhone 13+)
- [ ] Core ML integration for ANE acceleration
- [ ] Output: timestamped transcript as markdown file

**Success criteria**: Record 30s of speech, get readable transcript in <3s on iPhone 13.

### Phase 2 — Feature Parity with Voice Memo Flow (2-3 weeks)

**Goal**: Replace "record in Voice Memos → sync → minutes watch" with native app.

- [ ] `minutes-mobile` Rust crate (markdown write, frontmatter, config)
- [ ] Compile for `aarch64-apple-ios`, link as static lib
- [ ] Full pipeline: capture → transcribe → markdown → save
- [ ] Timestamped notes UI during recording
- [ ] Meeting list (browse local meetings via `minutes-reader`)
- [ ] Meeting detail view (render markdown)
- [ ] iCloud Drive sync (bidirectional)
- [ ] Background audio recording (record while app is backgrounded)

**Success criteria**: Record a meeting on phone, see it in Claude Desktop via MCP on Mac.

### Phase 3 — Polish + Android (3-4 weeks)

- [ ] Live transcription preview (stream partial results during recording)
- [ ] Audio level waveform visualization
- [ ] Widget: quick-record from home screen / lock screen
- [ ] Watch app: record from Apple Watch
- [ ] Optional cloud summarization (call LLM API from phone)
- [ ] Android port (Kotlin shell + whisper.cpp JNI + same Rust core)
- [ ] Model download manager (tiny bundled, base/small downloadable)

### Phase 4 — Advanced (future)

- [ ] On-device diarization (WhisperKit or sherpa-onnx)
- [ ] On-device summarization (small local LLM — Phi-3, Gemma)
- [ ] Calendar integration (iOS EventKit → auto-title meetings)
- [ ] Siri Shortcut: "Hey Siri, start Minutes"
- [ ] Live Activities (Dynamic Island shows recording status)
- [ ] Shared meeting rooms (multiple phones → one meeting)

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| whisper.cpp iOS build issues | Low | High | Official examples exist, well-trodden path |
| Tiny model accuracy too low | Medium | Medium | Fall back to base model (142MB), still fits on phone |
| Background recording killed by iOS | Medium | High | Use AVAudioSession `.record` category, test extensively |
| Memory pressure on older devices | Medium | Medium | Stick to tiny model, set minimum device to iPhone 12 |
| iCloud sync conflicts | Low | Medium | Last-write-wins for markdown (text merge is safe) |
| Core ML model conversion | Low | Medium | whisper.cpp has documented coreml scripts |
| App Store review (always-on mic) | Low | High | Clear privacy policy, no cloud upload, local-only processing |

---

## Open Questions

1. **Swift vs Tauri Mobile?** Swift gives native feel + better AVAudioEngine access. Tauri Mobile is faster to build but less polished. Recommendation: Swift for iOS, Kotlin for Android.

2. **Bundle model or download?** Tiny (75MB) is small enough to bundle. Larger models download on demand. App Store limits: 200MB OTA download threshold.

3. **Real-time streaming transcription?** whisper.cpp supports chunked processing. Could show live captions during recording. Nice-to-have for v2, not blocking for v1.

4. **Rust in the app?** For v1, could skip the Rust crate entirely — Swift can write YAML frontmatter + markdown directly. Add Rust later when sharing logic matters (Android port). Trade-off: faster v1 vs cleaner v2.

5. **Minimum iOS version?** iOS 16+ for AVAudioEngine improvements. iPhone 12+ for adequate whisper.cpp performance.
