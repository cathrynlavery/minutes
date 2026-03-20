use std::process::Command;

// ──────────────────────────────────────────────────────────────
// Calendar integration — upcoming meetings from macOS Calendar.
//
// Uses osascript (AppleScript) to query Calendar.app events.
// Returns upcoming events within a time window so the Tauri app
// can suggest "Start recording?" before meetings begin.
//
// Future: replace with EventKit via swift bridge for faster queries
// and proper permission handling from the .app bundle.
// ──────────────────────────────────────────────────────────────

/// A calendar event with title and start time.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CalendarEvent {
    pub title: String,
    pub start: String,
    pub minutes_until: i64,
}

/// Query upcoming calendar events within the next `lookahead_minutes`.
/// Returns events sorted by start time.
pub fn upcoming_events(lookahead_minutes: u32) -> Vec<CalendarEvent> {
    let script = format!(
        r#"
        use AppleScript version "2.4"
        set now to current date
        set horizon to now + ({minutes} * 60)
        set output to ""
        tell application "Calendar"
            repeat with cal in calendars
                try
                    repeat with evt in (every event of cal whose start date >= now and start date <= horizon)
                        set t to summary of evt
                        set s to start date of evt
                        set mins to ((s - now) / 60) as integer
                        set output to output & t & "␞" & (s as string) & "␞" & mins & linefeed
                    end repeat
                end try
            end repeat
        end tell
        return output
        "#,
        minutes = lookahead_minutes
    );

    let output = Command::new("osascript").arg("-e").arg(&script).output();

    let output = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return Vec::new(),
    };

    let mut events: Vec<CalendarEvent> = output
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(3, '␞').collect();
            if parts.len() >= 3 {
                Some(CalendarEvent {
                    title: parts[0].trim().to_string(),
                    start: parts[1].trim().to_string(),
                    minutes_until: parts[2].trim().parse().unwrap_or(0),
                })
            } else {
                None
            }
        })
        .collect();

    events.sort_by_key(|e| e.minutes_until);
    events
}
