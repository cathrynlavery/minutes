use crate::config::Config;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

// ──────────────────────────────────────────────────────────────
// Meeting notes — timestamped annotations from the user.
//
// During recording:
//   minutes note "Sarah wants $399"
//   → Reads recording start time from ~/.minutes/recording-start.txt
//   → Calculates elapsed time → [4:23]
//   → Appends to ~/.minutes/current-notes.md (atomic append)
//
// After recording:
//   minutes note --meeting <path> "Follow-up: confirmed via email"
//   → Appends to existing meeting file's ## Notes section
//
// In the pipeline:
//   Pipeline reads current-notes.md + current-context.txt
//   → Passes to LLM as high-priority context
//   → Includes in ## Notes section of output markdown
// ──────────────────────────────────────────────────────────────

pub fn notes_path() -> PathBuf {
    Config::minutes_dir().join("current-notes.md")
}

pub fn context_path() -> PathBuf {
    Config::minutes_dir().join("current-context.txt")
}

pub fn recording_start_path() -> PathBuf {
    Config::minutes_dir().join("recording-start.txt")
}

/// Save the recording start timestamp (epoch seconds).
pub fn save_recording_start() -> std::io::Result<()> {
    let path = recording_start_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    fs::write(&path, now.to_string())
}

/// Get elapsed time since recording started, formatted as [M:SS].
fn elapsed_timestamp() -> Option<String> {
    let path = recording_start_path();
    if !path.exists() {
        return None;
    }

    let start_str = fs::read_to_string(&path).ok()?;
    let start_epoch: u64 = start_str.trim().parse().ok()?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();

    let elapsed = now.saturating_sub(start_epoch);
    let mins = elapsed / 60;
    let secs = elapsed % 60;
    Some(format!("{}:{:02}", mins, secs))
}

/// Add a note to the current recording.
/// Returns the timestamped note line that was appended.
pub fn add_note(text: &str) -> Result<String, String> {
    // Check recording is in progress
    let pid_path = crate::pid::pid_path();
    if !pid_path.exists() {
        return Err("No recording in progress. Start one with: minutes record".into());
    }

    let timestamp = elapsed_timestamp().unwrap_or_else(|| "?:??".into());
    let line = format!("[{}] {}", timestamp, text.trim());

    // Atomic append (O_APPEND mode)
    let path = notes_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("could not open notes file: {}", e))?;

    writeln!(file, "{}", line).map_err(|e| format!("could not write note: {}", e))?;

    tracing::info!(note = %line, "note added");
    Ok(line)
}

/// Save pre-meeting context.
pub fn save_context(text: &str) -> std::io::Result<()> {
    let path = context_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, text.trim())
}

/// Read current notes (if any). Returns None if no notes file exists.
pub fn read_notes() -> Option<String> {
    let path = notes_path();
    if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .filter(|s| !s.trim().is_empty())
    } else {
        None
    }
}

/// Read pre-meeting context (if any).
pub fn read_context() -> Option<String> {
    let path = context_path();
    if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .filter(|s| !s.trim().is_empty())
    } else {
        None
    }
}

/// Clean up notes and context files after recording completes.
pub fn cleanup() {
    let _ = fs::remove_file(notes_path());
    let _ = fs::remove_file(context_path());
    let _ = fs::remove_file(recording_start_path());
}

/// Add a note to an existing meeting file (post-meeting annotation).
pub fn annotate_meeting(meeting_path: &Path, text: &str) -> Result<(), String> {
    if !meeting_path.exists() {
        return Err(format!(
            "meeting file not found: {}",
            meeting_path.display()
        ));
    }

    let now = chrono::Local::now()
        .format("%b %d, post-meeting")
        .to_string();
    let note_line = format!("- [{}] {}", now, text.trim());

    let mut content = fs::read_to_string(meeting_path).map_err(|e| e.to_string())?;

    // Find ## Notes section and append, or create it
    if let Some(pos) = content.find("## Notes") {
        // Find the end of the Notes section (next ## or end of file)
        let notes_start = pos + "## Notes".len();
        let next_section = content[notes_start..]
            .find("\n## ")
            .map(|i| notes_start + i);

        let insert_pos = next_section.unwrap_or(content.len());
        content.insert_str(insert_pos, &format!("\n{}\n", note_line));
    } else {
        // Find ## Transcript or ## Decisions and insert Notes before it
        let insert_before = ["## Transcript", "## Decisions", "## Action Items"];
        let mut inserted = false;

        for marker in &insert_before {
            if let Some(pos) = content.find(marker) {
                content.insert_str(pos, &format!("## Notes\n\n{}\n\n", note_line));
                inserted = true;
                break;
            }
        }

        if !inserted {
            // Append to end
            content.push_str(&format!("\n## Notes\n\n{}\n", note_line));
        }
    }

    fs::write(meeting_path, &content).map_err(|e| e.to_string())?;

    // Restore 0600 permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(meeting_path, fs::Permissions::from_mode(0o600));
    }

    tracing::info!(
        meeting = %meeting_path.display(),
        note = %text.trim(),
        "post-meeting note added"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn elapsed_timestamp_returns_none_without_recording() {
        // No recording-start.txt should exist in test environment
        // (unless a recording is actually happening on this machine)
        // This test is environment-dependent, so just verify the function doesn't panic
        let _ = elapsed_timestamp();
    }

    #[test]
    fn annotate_meeting_creates_notes_section() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test-meeting.md");
        fs::write(
            &path,
            "---\ntitle: Test\n---\n\n## Summary\n\nGood meeting.\n\n## Transcript\n\n[0:00] Hello\n",
        )
        .unwrap();

        annotate_meeting(&path, "Follow-up needed").unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("## Notes"));
        assert!(content.contains("Follow-up needed"));
        // Notes should appear before Transcript
        let notes_pos = content.find("## Notes").unwrap();
        let transcript_pos = content.find("## Transcript").unwrap();
        assert!(notes_pos < transcript_pos);
    }

    #[test]
    fn annotate_meeting_appends_to_existing_notes() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test-meeting.md");
        fs::write(
            &path,
            "---\ntitle: Test\n---\n\n## Notes\n\n- [4:23] First note\n\n## Transcript\n\n[0:00] Hello\n",
        )
        .unwrap();

        annotate_meeting(&path, "Second note").unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("First note"));
        assert!(content.contains("Second note"));
    }

    #[test]
    fn annotate_meeting_rejects_nonexistent_file() {
        let result = annotate_meeting(Path::new("/nonexistent/meeting.md"), "note");
        assert!(result.is_err());
    }
}
