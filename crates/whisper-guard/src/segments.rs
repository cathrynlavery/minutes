//! Post-transcription segment cleaning for whisper output.
//!
//! Whisper's decoder can hallucinate in several patterns:
//! - **Consecutive repetition**: the same phrase repeated 5-50 times
//! - **Interleaved repetition**: A/B/A/B patterns with filler words between
//! - **Trailing noise**: `[music]`, `[BLANK_AUDIO]` tags after speech ends
//!
//! This module detects and removes all three patterns. The main entry point
//! is [`clean_transcript`], which chains all cleaning passes.

/// Extract the text portion after the timestamp bracket.
/// Lines look like `[0:00] some text` or plain text.
fn text_part(line: &str) -> &str {
    line.find("] ").map(|i| &line[i + 2..]).unwrap_or(line)
}

/// Statistics from transcript cleaning.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CleanStats {
    pub original_lines: usize,
    pub after_consecutive_dedup: usize,
    pub after_interleaved_dedup: usize,
    pub after_trailing_trim: usize,
    pub lines_removed: usize,
}

/// Clean an existing transcript by running all post-processing dedup layers.
///
/// Takes the raw transcript text (lines like `[0:00] some text`) and returns
/// cleaned text with hallucination patterns removed, plus statistics.
///
/// This is idempotent: running it on already-cleaned text produces the same output.
pub fn clean_transcript(transcript: &str) -> (String, CleanStats) {
    let lines: Vec<String> = transcript.lines().map(|l| l.to_string()).collect();
    let original_count = lines.len();

    let lines = dedup_segments(&lines);
    let after_consecutive = lines.len();

    let lines = dedup_interleaved(&lines);
    let after_interleaved = lines.len();

    let lines = trim_trailing_noise(&lines);
    let after_trim = lines.len();

    let stats = CleanStats {
        original_lines: original_count,
        after_consecutive_dedup: after_consecutive,
        after_interleaved_dedup: after_interleaved,
        after_trailing_trim: after_trim,
        lines_removed: original_count.saturating_sub(after_trim),
    };

    (lines.join("\n"), stats)
}

/// Detect and remove repetition loops from whisper output.
///
/// Whisper's decoder can get stuck repeating the same text across consecutive segments,
/// especially on non-English audio. This function detects runs of 3+ consecutive segments
/// with >80% text overlap and collapses them to the first occurrence.
pub fn dedup_segments(lines: &[String]) -> Vec<String> {
    if lines.len() < 3 {
        return lines.to_vec();
    }

    // Simple text similarity: ratio of matching chars to total chars (normalized)
    fn similarity(a: &str, b: &str) -> f64 {
        if a.is_empty() || b.is_empty() {
            return 0.0;
        }
        let a_lower = a.to_lowercase();
        let b_lower = b.to_lowercase();
        if a_lower == b_lower {
            return 1.0;
        }
        // Use longest common substring ratio as a fast similarity measure
        let (short, long) = if a_lower.len() <= b_lower.len() {
            (&a_lower, &b_lower)
        } else {
            (&b_lower, &a_lower)
        };
        if long.contains(short.as_str()) {
            return short.len() as f64 / long.len() as f64;
        }
        // Count matching words as fallback
        let a_words: Vec<&str> = a_lower.split_whitespace().collect();
        let b_words: Vec<&str> = b_lower.split_whitespace().collect();
        let matching = a_words.iter().filter(|w| b_words.contains(w)).count();
        let total = a_words.len().max(b_words.len());
        if total == 0 {
            return 0.0;
        }
        matching as f64 / total as f64
    }

    let mut result = Vec::with_capacity(lines.len());
    let mut i = 0;

    while i < lines.len() {
        let base_text = text_part(&lines[i]);
        let mut run_end = i + 1;

        while run_end < lines.len() {
            let candidate = text_part(&lines[run_end]);
            if similarity(base_text, candidate) >= 0.8 {
                run_end += 1;
            } else {
                break;
            }
        }

        let run_len = run_end - i;

        if run_len >= 3 {
            tracing::debug!(
                first_segment = i,
                repeated_count = run_len,
                text = base_text,
                "detected repetition loop in whisper output — collapsing {} segments",
                run_len
            );
            result.push(lines[i].clone());
            result.push(format!(
                "[...] [repeated audio removed — {} identical segments collapsed]",
                run_len - 1
            ));
            i = run_end;
        } else {
            result.push(lines[i].clone());
            i += 1;
        }
    }

    result
}

/// Detect interleaved repetition patterns that escape consecutive dedup.
///
/// Whisper often hallucinates alternating patterns like:
///   "So I'm going to pick his brain" / "Okay." / "So I'm going to pick his brain" / "Okay."
/// or inserts short filler between repeated phrases. The consecutive dedup misses these
/// because no two adjacent lines are similar.
///
/// Strategy: use a sliding window to detect when a single phrase dominates a region.
/// If any phrase appears in >=50% of lines within a 10-line window, and the window
/// contains at least 5 such occurrences, collapse the entire dominated region.
pub fn dedup_interleaved(lines: &[String]) -> Vec<String> {
    if lines.len() < 6 {
        return lines.to_vec();
    }

    fn normalize(text: &str) -> String {
        text.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Short filler phrases that whisper inserts between hallucinated repetitions.
    fn is_filler(text: &str) -> bool {
        let normalized = text.trim().to_lowercase();
        let normalized = normalized.trim_matches(|c: char| !c.is_alphanumeric());
        matches!(
            normalized,
            "okay"
                | "ok"
                | "yeah"
                | "yes"
                | "right"
                | "so"
                | "and"
                | "but"
                | "well"
                | "uh"
                | "um"
                | "hmm"
                | "mhm"
        )
    }

    // Build normalized text for each line
    let texts: Vec<String> = lines.iter().map(|l| normalize(text_part(l))).collect();
    let fillers: Vec<bool> = texts.iter().map(|t| is_filler(t)).collect();

    // Mark lines that are part of a hallucination region.
    let mut remove = vec![false; lines.len()];

    let window_size = 10;
    let min_occurrences = 5;

    let mut i = 0;
    while i + window_size <= lines.len() {
        // Count phrase frequencies in this window (excluding fillers)
        let mut freq: std::collections::BTreeMap<&str, Vec<usize>> =
            std::collections::BTreeMap::new();
        for j in i..i + window_size {
            if !fillers[j] && !texts[j].is_empty() {
                freq.entry(&texts[j]).or_default().push(j);
            }
        }

        // Find the dominant phrase (BTreeMap for deterministic iteration order)
        let dominant = freq
            .iter()
            .max_by(|(phrase_a, pos_a), (phrase_b, pos_b)| {
                pos_a
                    .len()
                    .cmp(&pos_b.len())
                    .then_with(|| phrase_a.cmp(phrase_b))
            })
            .filter(|(_, positions)| positions.len() >= min_occurrences);

        if let Some((phrase, _)) = dominant {
            let phrase = phrase.to_string();
            // Extend the region: keep scanning forward while the phrase keeps appearing
            let mut region_end = i + window_size;
            while region_end < lines.len() {
                let t = &texts[region_end];
                if *t == phrase || fillers[region_end] {
                    region_end += 1;
                } else {
                    let mut gap = 0;
                    let mut found_resume = false;
                    for t in texts
                        .iter()
                        .take(lines.len().min(region_end + 3))
                        .skip(region_end)
                    {
                        if *t == phrase {
                            found_resume = true;
                            break;
                        }
                        gap += 1;
                    }
                    if found_resume && gap <= 2 {
                        region_end += gap + 1;
                    } else {
                        break;
                    }
                }
            }

            let region_len = region_end - i;
            let actual_count = (i..region_end).filter(|&j| texts[j] == phrase).count();

            if actual_count >= min_occurrences && region_len >= 6 {
                tracing::debug!(
                    region_start = i,
                    region_end = region_end,
                    occurrences = actual_count,
                    filler_count = (i..region_end).filter(|&j| fillers[j]).count(),
                    phrase = phrase,
                    "detected interleaved hallucination loop — marking {} lines for removal",
                    region_len
                );
                let mut kept_first = false;
                for j in i..region_end {
                    if !kept_first && texts[j] == phrase {
                        kept_first = true;
                    } else {
                        remove[j] = true;
                    }
                }
                i = region_end;
                continue;
            }
        }

        i += 1;
    }

    let removed_count = remove.iter().filter(|&&r| r).count();
    if removed_count > 0 {
        let mut result = Vec::with_capacity(lines.len() - removed_count + 1);
        let mut in_removed_run = false;

        for (idx, line) in lines.iter().enumerate() {
            if remove[idx] {
                if !in_removed_run {
                    in_removed_run = true;
                    let run_len = (idx..lines.len()).take_while(|&j| remove[j]).count();
                    result.push(format!(
                        "[...] [hallucinated repetition removed — {} lines collapsed]",
                        run_len
                    ));
                }
            } else {
                in_removed_run = false;
                result.push(line.clone());
            }
        }

        tracing::info!(
            original = lines.len(),
            removed = removed_count,
            remaining = result.len(),
            "interleaved dedup complete"
        );
        result
    } else {
        lines.to_vec()
    }
}

/// Trim trailing non-speech noise from the end of a transcript.
///
/// Recordings often capture music, silence, or ambient noise after the conversation
/// ends. Long runs of `[music]`, `[BLANK_AUDIO]`, or very short filler at the end
/// add no value and make the transcript look broken.
pub fn trim_trailing_noise(lines: &[String]) -> Vec<String> {
    if lines.is_empty() {
        return Vec::new();
    }

    fn is_noise(text: &str) -> bool {
        let t = text.trim().to_lowercase();
        t == "[music]"
            || t == "[blank_audio]"
            || t == "[silence]"
            || t == "music"
            || t == "you"                // common whisper hallucination on silence
            || t == "okay."
            || t == "yeah."
        // Note: collapse markers ("[...] [repeated ...]") are NOT noise —
        // treating them as noise would make clean_transcript non-idempotent.
    }

    // Walk backward from the end, counting consecutive noise lines
    let mut trim_from = lines.len();
    for i in (0..lines.len()).rev() {
        let text = text_part(&lines[i]);
        if is_noise(text) {
            trim_from = i;
        } else {
            break;
        }
    }

    // Only trim if we're removing a significant trailing block (5+ lines)
    let trimmed_count = lines.len() - trim_from;
    if trimmed_count >= 5 {
        tracing::info!(
            trimmed = trimmed_count,
            "removed trailing noise from transcript"
        );
        let mut result: Vec<String> = lines[..trim_from].to_vec();
        result.push(format!(
            "[Recording ended — {} lines of trailing noise removed]",
            trimmed_count
        ));
        result
    } else {
        lines.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── clean_transcript end-to-end ──

    #[test]
    fn clean_transcript_removes_repetition() {
        let input = "[0:00] Hello world\n[0:03] Hello world\n[0:06] Hello world\n[0:09] Hello world\n[0:12] Something different\n";
        let (cleaned, stats) = clean_transcript(input);
        assert!(stats.lines_removed > 0);
        assert!(cleaned.contains("Something different"));
        assert!(cleaned.contains("repeated audio removed"));
    }

    #[test]
    fn clean_transcript_preserves_normal_text() {
        let input = "[0:00] First line\n[0:05] Second line\n[0:10] Third line\n";
        let (cleaned, stats) = clean_transcript(input);
        assert_eq!(stats.lines_removed, 0);
        assert!(cleaned.contains("First line"));
        assert!(cleaned.contains("Third line"));
    }

    // ── dedup_segments ──

    #[test]
    fn dedup_no_repetition() {
        let lines = vec![
            "[0:00] Hello world".into(),
            "[0:03] How are you".into(),
            "[0:06] Fine thanks".into(),
        ];
        let result = dedup_segments(&lines);
        assert_eq!(result, lines);
    }

    #[test]
    fn dedup_collapses_exact_repetition() {
        let lines = vec![
            "[0:00] Hello world".into(),
            "[0:03] Hello world".into(),
            "[0:06] Hello world".into(),
            "[0:09] Hello world".into(),
            "[0:12] Something different".into(),
        ];
        let result = dedup_segments(&lines);
        assert_eq!(result.len(), 3);
        assert!(result[0].contains("Hello world"));
        assert!(result[1].contains("repeated audio removed"));
        assert!(result[2].contains("Something different"));
    }

    #[test]
    fn dedup_collapses_near_identical() {
        let lines = vec![
            "[0:00] Ok bene le macedi diesel".into(),
            "[0:03] Ok, bene le macedi diesel".into(),
            "[0:06] Ok bene, le macedi diesel".into(),
            "[0:09] Good morning".into(),
        ];
        let result = dedup_segments(&lines);
        assert_eq!(result.len(), 3);
        assert!(result[1].contains("repeated audio removed"));
    }

    #[test]
    fn dedup_leaves_two_similar_alone() {
        let lines = vec![
            "[0:00] Hello world".into(),
            "[0:03] Hello world".into(),
            "[0:06] Something else".into(),
        ];
        let result = dedup_segments(&lines);
        assert_eq!(result, lines);
    }

    #[test]
    fn dedup_handles_empty() {
        let result = dedup_segments(&vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn dedup_handles_single_line() {
        let lines = vec!["[0:00] Hello".into()];
        let result = dedup_segments(&lines);
        assert_eq!(result, lines);
    }

    #[test]
    fn dedup_multiple_runs() {
        let lines = vec![
            "[0:00] First phrase".into(),
            "[0:03] First phrase".into(),
            "[0:06] First phrase".into(),
            "[0:09] Second phrase".into(),
            "[0:12] Second phrase".into(),
            "[0:15] Second phrase".into(),
            "[0:18] Second phrase".into(),
            "[0:21] Normal text".into(),
        ];
        let result = dedup_segments(&lines);
        assert_eq!(result.len(), 5);
        assert!(result[1].contains("2 identical"));
        assert!(result[3].contains("3 identical"));
    }

    // ── interleaved dedup ──

    #[test]
    fn interleaved_catches_alternating_pattern() {
        let mut lines: Vec<String> = Vec::new();
        for i in 0..20 {
            let ts = i * 2;
            if i % 2 == 0 {
                lines.push(format!(
                    "[{}:{:02}] So I'm going to pick his brain as well.",
                    ts / 60,
                    ts % 60
                ));
            } else {
                lines.push(format!("[{}:{:02}] Okay.", ts / 60, ts % 60));
            }
        }
        lines.push("[0:40] Something completely different".into());

        let result = dedup_interleaved(&lines);
        assert!(
            result.len() <= 4,
            "expected <=4 lines, got {}: {:?}",
            result.len(),
            result
        );
        assert!(result.iter().any(|l| l.contains("pick his brain")));
        assert!(result
            .iter()
            .any(|l| l.contains("hallucinated repetition removed")));
        assert!(result
            .last()
            .unwrap()
            .contains("Something completely different"));
    }

    #[test]
    fn interleaved_leaves_normal_conversation() {
        let lines = vec![
            "[0:00] Hello how are you".into(),
            "[0:05] I'm fine thanks".into(),
            "[0:10] Great to hear".into(),
            "[0:15] Let's talk about the project".into(),
            "[0:20] Sure what's the update".into(),
            "[0:25] We shipped the feature".into(),
        ];
        let result = dedup_interleaved(&lines);
        assert_eq!(result, lines);
    }

    #[test]
    fn interleaved_ignores_short_repeats() {
        let lines = vec![
            "[0:00] Hello world".into(),
            "[0:02] Okay.".into(),
            "[0:04] Hello world".into(),
            "[0:06] Okay.".into(),
            "[0:08] Hello world".into(),
            "[0:10] Something else".into(),
        ];
        let result = dedup_interleaved(&lines);
        assert_eq!(result, lines);
    }

    // ── trailing noise ──

    #[test]
    fn trim_trailing_music() {
        let mut lines: Vec<String> = vec![
            "[0:00] Hello world".into(),
            "[0:05] Some real content".into(),
        ];
        for i in 0..20 {
            lines.push(format!("[{}:00] [music]", i + 1));
        }
        let result = trim_trailing_noise(&lines);
        assert_eq!(result.len(), 3);
        assert!(result[0].contains("Hello world"));
        assert!(result[1].contains("real content"));
        assert!(result[2].contains("trailing noise removed"));
    }

    #[test]
    fn trim_keeps_short_trailing_noise() {
        let lines = vec![
            "[0:00] Hello world".into(),
            "[0:05] [music]".into(),
            "[0:10] [music]".into(),
            "[0:15] [music]".into(),
        ];
        let result = trim_trailing_noise(&lines);
        assert_eq!(result, lines);
    }

    #[test]
    fn trim_handles_empty() {
        assert!(trim_trailing_noise(&vec![]).is_empty());
    }

    #[test]
    fn trim_all_noise() {
        let lines: Vec<String> = (0..10).map(|i| format!("[{}:00] [music]", i)).collect();
        let result = trim_trailing_noise(&lines);
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("trailing noise removed"));
    }
}
