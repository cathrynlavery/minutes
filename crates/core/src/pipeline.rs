use crate::config::Config;
use crate::diarize;
use crate::error::MinutesError;
use crate::logging;
use crate::markdown::{self, ContentType, Frontmatter, OutputStatus, WriteResult};
use crate::notes;
use crate::summarize;
use crate::transcribe;
use chrono::Local;
use std::path::Path;

// ──────────────────────────────────────────────────────────────
// Pipeline orchestration:
//
//   Audio → Transcribe → [Diarize] → [Summarize] → Write Markdown
//                           ▲             ▲
//                           │             │
//                     config.diarization  config.summarization
//                     .engine != "none"   .engine != "none"
//
// Transcription uses whisper-rs (whisper.cpp) with symphonia for
// format conversion (m4a/mp3/ogg → 16kHz mono PCM).
// Phase 1b adds Diarize + Summarize with if-guards.
// ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PipelineStage {
    Transcribing,
    Diarizing,
    Summarizing,
    Saving,
}

/// Process an audio file through the full pipeline.
pub fn process(
    audio_path: &Path,
    content_type: ContentType,
    title: Option<&str>,
    config: &Config,
) -> Result<WriteResult, MinutesError> {
    process_with_progress(audio_path, content_type, title, config, |_| {})
}

pub fn process_with_progress<F>(
    audio_path: &Path,
    content_type: ContentType,
    title: Option<&str>,
    config: &Config,
    mut on_progress: F,
) -> Result<WriteResult, MinutesError>
where
    F: FnMut(PipelineStage),
{
    let start = std::time::Instant::now();
    tracing::info!(
        file = %audio_path.display(),
        content_type = ?content_type,
        "starting pipeline"
    );

    // Verify file exists and is not empty
    let metadata = std::fs::metadata(audio_path)?;
    if metadata.len() == 0 {
        return Err(crate::error::TranscribeError::EmptyAudio.into());
    }

    // Security: verify file is in an allowed directory (prevents path traversal via MCP)
    if let Ok(canonical) = audio_path.canonicalize() {
        let allowed = &config.security.allowed_audio_dirs;
        if !allowed.is_empty() {
            let in_allowed = allowed.iter().any(|dir| {
                dir.canonicalize()
                    .map(|d| canonical.starts_with(&d))
                    .unwrap_or(false)
            });
            if !in_allowed {
                return Err(crate::error::TranscribeError::UnsupportedFormat(format!(
                    "file not in allowed directories: {}",
                    audio_path.display()
                ))
                .into());
            }
        }
    }

    // Step 1: Transcribe (always)
    on_progress(PipelineStage::Transcribing);
    tracing::info!(step = "transcribe", file = %audio_path.display(), "transcribing audio");
    let step_start = std::time::Instant::now();
    let transcript = transcribe::transcribe(audio_path, config)?;
    let transcribe_ms = step_start.elapsed().as_millis() as u64;

    let word_count = transcript.split_whitespace().count();
    tracing::info!(
        step = "transcribe",
        words = word_count,
        "transcription complete"
    );
    logging::log_step(
        "transcribe",
        &audio_path.display().to_string(),
        transcribe_ms,
        serde_json::json!({"words": word_count}),
    );

    // Check minimum word threshold
    let status = if word_count < config.transcription.min_words {
        tracing::warn!(
            words = word_count,
            min = config.transcription.min_words,
            "below minimum word threshold — marking as no-speech"
        );
        Some(OutputStatus::NoSpeech)
    } else if config.summarization.engine != "none" {
        Some(OutputStatus::Complete)
    } else {
        Some(OutputStatus::TranscriptOnly)
    };

    // Step 2: Diarize (optional — depends on config.diarization.engine)
    let transcript = if config.diarization.engine != "none" {
        on_progress(PipelineStage::Diarizing);
        tracing::info!(step = "diarize", "running speaker diarization");
        if let Some(result) = diarize::diarize(audio_path, config) {
            diarize::apply_speakers(&transcript, &result)
        } else {
            transcript
        }
    } else {
        transcript
    };

    // Read user notes and pre-meeting context (if any)
    let user_notes = notes::read_notes();
    let pre_context = notes::read_context();

    // Step 3: Summarize (optional — depends on config.summarization.engine)
    // Pass user notes to the summarizer as high-priority context
    // Step 3: Summarize + extract structured intent
    let mut structured_actions: Vec<markdown::ActionItem> = Vec::new();
    let mut structured_decisions: Vec<markdown::Decision> = Vec::new();

    let summary: Option<String> = if config.summarization.engine != "none" {
        on_progress(PipelineStage::Summarizing);
        tracing::info!(step = "summarize", "generating summary");
        let transcript_with_notes = if let Some(ref n) = user_notes {
            format!(
                "USER NOTES (these moments were marked as important — weight them heavily):\n{}\n\n\
                 TRANSCRIPT:\n{}",
                n, transcript
            )
        } else {
            transcript.clone()
        };
        summarize::summarize(&transcript_with_notes, config).map(|s| {
            // Extract structured data from the summary
            structured_actions = extract_action_items(&s);
            structured_decisions = extract_decisions(&s);
            summarize::format_summary(&s)
        })
    } else {
        None
    };

    // Step 4: Write markdown (always)
    on_progress(PipelineStage::Saving);
    let duration = estimate_duration(audio_path);
    let auto_title = title
        .map(String::from)
        .unwrap_or_else(|| generate_title(&transcript, pre_context.as_deref()));

    let frontmatter = Frontmatter {
        title: auto_title,
        r#type: content_type,
        date: Local::now(),
        duration,
        source: match content_type {
            ContentType::Memo => Some("voice-memo".into()),
            ContentType::Meeting => None,
        },
        status,
        tags: vec![],
        attendees: vec![],
        calendar_event: None,
        people: vec![],
        context: pre_context,
        action_items: structured_actions,
        decisions: structured_decisions,
    };

    tracing::info!(step = "write", "writing markdown");
    let step_start = std::time::Instant::now();
    let result = markdown::write(
        &frontmatter,
        &transcript,
        summary.as_deref(),
        user_notes.as_deref(),
        config,
    )?;
    let write_ms = step_start.elapsed().as_millis() as u64;
    logging::log_step(
        "write",
        &audio_path.display().to_string(),
        write_ms,
        serde_json::json!({"output": result.path.display().to_string(), "words": result.word_count}),
    );

    let elapsed = start.elapsed();
    logging::log_step(
        "pipeline_complete",
        &audio_path.display().to_string(),
        elapsed.as_millis() as u64,
        serde_json::json!({"output": result.path.display().to_string(), "words": result.word_count, "content_type": format!("{:?}", content_type)}),
    );
    tracing::info!(
        file = %result.path.display(),
        words = result.word_count,
        elapsed_ms = elapsed.as_millis() as u64,
        "pipeline complete"
    );

    Ok(result)
}

/// Estimate audio duration from file size (rough approximation).
/// 16kHz mono 16-bit WAV ≈ 32KB/sec.
fn estimate_duration(audio_path: &Path) -> String {
    let bytes = std::fs::metadata(audio_path).map(|m| m.len()).unwrap_or(0);

    // WAV header is 44 bytes, then raw PCM at 32000 bytes/sec (16kHz 16-bit mono)
    let secs = if bytes > 44 { (bytes - 44) / 32_000 } else { 0 };

    let mins = secs / 60;
    let remaining_secs = secs % 60;
    if mins > 0 {
        format!("{}m {}s", mins, remaining_secs)
    } else {
        format!("{}s", remaining_secs)
    }
}

/// Generate a smart title from either the user-provided context or transcript.
fn generate_title(transcript: &str, pre_context: Option<&str>) -> String {
    if let Some(context) = pre_context.and_then(title_from_context) {
        return finalize_title(context);
    }

    if let Some(transcript_title) = title_from_transcript(transcript) {
        return finalize_title(transcript_title);
    }

    "Untitled Recording".into()
}

fn title_from_context(context: &str) -> Option<String> {
    let cleaned = normalize_space(context);
    if cleaned.is_empty() {
        return None;
    }

    let lower = cleaned.to_lowercase();
    let generic = [
        "meeting",
        "recording",
        "memo",
        "voice memo",
        "call",
        "conversation",
        "note",
    ];
    if generic.contains(&lower.as_str()) {
        return None;
    }

    Some(to_display_title(&cleaned))
}

fn title_from_transcript(transcript: &str) -> Option<String> {
    let first_line = transcript.lines().find_map(clean_transcript_line)?;
    let stripped = strip_lead_in_phrase(&first_line);
    let candidate = normalize_space(&stripped);

    if candidate.is_empty() {
        None
    } else {
        Some(to_display_title(&candidate))
    }
}

fn clean_transcript_line(line: &str) -> Option<String> {
    let mut remaining = line.trim();

    while let Some(rest) = remaining.strip_prefix('[') {
        let bracket_end = rest.find(']')?;
        remaining = rest[bracket_end + 1..].trim();
    }

    let cleaned = normalize_space(remaining);
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn strip_lead_in_phrase(line: &str) -> String {
    let cleaned = normalize_space(line);
    let lower = cleaned.to_lowercase();
    let prefixes = [
        "we need to discuss ",
        "let's talk about ",
        "lets talk about ",
        "let's discuss ",
        "lets discuss ",
        "i just had an idea about ",
        "i had an idea about ",
        "this is about ",
        "today we're talking about ",
        "today we are talking about ",
        "we're talking about ",
        "we are talking about ",
        "we should talk about ",
        "we should discuss ",
        "i want to talk about ",
        "i want to discuss ",
    ];

    for prefix in prefixes {
        if lower.starts_with(prefix) {
            return cleaned[prefix.len()..].trim().to_string();
        }
    }

    cleaned
}

fn normalize_space(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn to_display_title(text: &str) -> String {
    let trimmed = text
        .trim_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace())
        .split(['.', '!', '?', '\n'])
        .next()
        .unwrap_or("")
        .trim();

    let stopwords = [
        "a", "an", "and", "as", "at", "by", "for", "from", "in", "of", "on", "or", "the", "to",
        "with",
    ];

    trimmed
        .split_whitespace()
        .enumerate()
        .map(|(idx, word)| {
            let lower = word.to_lowercase();
            let is_edge = idx == 0;
            if word.chars().any(|c| c.is_ascii_digit())
                || word
                    .chars()
                    .all(|c| !c.is_ascii_lowercase() || !c.is_ascii_uppercase())
                    && word.chars().filter(|c| c.is_ascii_uppercase()).count() > 1
            {
                word.to_string()
            } else if !is_edge && stopwords.contains(&lower.as_str()) {
                lower
            } else {
                let mut chars = lower.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn finalize_title(title: String) -> String {
    if title.chars().count() > 60 {
        let truncated: String = title.chars().take(57).collect();
        format!("{}...", truncated)
    } else {
        title
    }
}

/// Extract structured action items from a Summary.
/// Parses lines like "- @user: Send pricing doc by Friday" into ActionItem structs.
fn extract_action_items(summary: &summarize::Summary) -> Vec<markdown::ActionItem> {
    summary
        .action_items
        .iter()
        .map(|item| {
            let (assignee, task) = if let Some(rest) = item.strip_prefix('@') {
                // "@user: Send pricing doc by Friday"
                if let Some(colon_pos) = rest.find(':') {
                    (
                        rest[..colon_pos].trim().to_string(),
                        rest[colon_pos + 1..].trim().to_string(),
                    )
                } else {
                    ("unassigned".to_string(), item.clone())
                }
            } else {
                ("unassigned".to_string(), item.clone())
            };

            // Try to extract due date from phrases like "by Friday", "(due March 21)"
            let due = extract_due_date(&task);

            markdown::ActionItem {
                assignee,
                task: task.trim_end_matches(')').trim().to_string(),
                due,
                status: "open".to_string(),
            }
        })
        .collect()
}

/// Extract structured decisions from a Summary.
fn extract_decisions(summary: &summarize::Summary) -> Vec<markdown::Decision> {
    summary
        .decisions
        .iter()
        .map(|text| {
            // Try to infer topic from the first few words
            let topic = infer_topic(text);
            markdown::Decision {
                text: text.clone(),
                topic,
            }
        })
        .collect()
}

/// Try to extract a due date from action item text.
/// Matches patterns like "by Friday", "by March 21", "(due 2026-03-21)".
fn extract_due_date(text: &str) -> Option<String> {
    let lower = text.to_lowercase();

    // "by Friday", "by next week", "by March 21"
    if let Some(pos) = lower.find(" by ") {
        let after = &text[pos + 4..];
        let due: String = after
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
            .collect();
        let due = due.trim().to_string();
        if !due.is_empty() {
            return Some(due);
        }
    }

    // "(due March 21)"
    if let Some(pos) = lower.find("due ") {
        let after = &text[pos + 4..];
        let due: String = after
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
            .collect();
        let due = due.trim().to_string();
        if !due.is_empty() {
            return Some(due);
        }
    }

    None
}

/// Infer a topic from decision text by extracting the first noun phrase.
fn infer_topic(text: &str) -> Option<String> {
    // Simple heuristic: use the first 3-5 meaningful words as the topic
    let words: Vec<&str> = text
        .split_whitespace()
        .filter(|w| {
            let lower = w.to_lowercase();
            !matches!(
                lower.as_str(),
                "the"
                    | "a"
                    | "an"
                    | "to"
                    | "for"
                    | "of"
                    | "in"
                    | "on"
                    | "at"
                    | "is"
                    | "was"
                    | "will"
                    | "should"
                    | "we"
                    | "they"
                    | "it"
            )
        })
        .take(4)
        .collect();

    if words.is_empty() {
        None
    } else {
        Some(words.join(" ").to_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_title_takes_first_words() {
        let transcript = "We need to discuss the new pricing strategy for Q2";
        let title = generate_title(transcript, None);
        assert_eq!(title, "The New Pricing Strategy for Q2");
    }

    #[test]
    fn generate_title_strips_timestamps_and_speaker_labels() {
        let transcript = "[SPEAKER_0 0:00] let's talk about API launch timeline for Q2";
        let title = generate_title(transcript, None);
        assert_eq!(title, "Advisor Pricing for Q2");
    }

    #[test]
    fn generate_title_prefers_context_when_available() {
        let transcript = "Okay so I just had an idea about onboarding";
        let title = generate_title(transcript, Some("Q2 pricing discussion with Alex"));
        assert_eq!(title, "Q2 Pricing Discussion with Alex");
    }

    #[test]
    fn generate_title_falls_back_when_only_timestamps_exist() {
        let transcript = "[0:00]";
        let title = generate_title(transcript, None);
        assert_eq!(title, "Untitled Recording");
    }

    #[test]
    fn estimate_duration_formats_correctly() {
        // 32000 bytes/sec * 90 sec + 44 header = 2_880_044 bytes
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("test.wav");
        let data = vec![0u8; 2_880_044];
        std::fs::write(&path, &data).unwrap();

        let duration = estimate_duration(&path);
        assert_eq!(duration, "1m 30s");
    }

    #[test]
    fn extract_action_items_parses_assignee_and_task() {
        let summary = summarize::Summary {
            text: String::new(),
            key_points: vec![],
            decisions: vec![],
            action_items: vec![
                "@user: Send pricing doc by Friday".into(),
                "@sarah: Review competitor grid (due March 21)".into(),
                "Unassigned task with no @".into(),
            ],
        };

        let items = extract_action_items(&summary);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].assignee, "mat");
        assert!(items[0].task.contains("Send pricing doc"));
        assert_eq!(items[0].due, Some("Friday".into()));
        assert_eq!(items[0].status, "open");

        assert_eq!(items[1].assignee, "sarah");
        assert_eq!(items[1].due, Some("March 21".into()));

        assert_eq!(items[2].assignee, "unassigned");
    }

    #[test]
    fn extract_decisions_with_topic_inference() {
        let summary = summarize::Summary {
            text: String::new(),
            key_points: vec![],
            decisions: vec![
                "Price advisor platform at monthly billing/mo".into(),
                "Use REST over GraphQL for the new API".into(),
            ],
            action_items: vec![],
        };

        let decisions = extract_decisions(&summary);
        assert_eq!(decisions.len(), 2);
        assert!(decisions[0].topic.is_some());
        assert!(decisions[0].text.contains("monthly billing"));
    }

    #[test]
    fn extract_due_date_patterns() {
        assert_eq!(
            extract_due_date("Send doc by Friday"),
            Some("Friday".into())
        );
        assert_eq!(
            extract_due_date("Review (due March 21)"),
            Some("March 21".into())
        );
        assert_eq!(extract_due_date("Just do this thing"), None);
    }
}
