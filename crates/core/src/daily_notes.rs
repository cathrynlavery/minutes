use crate::config::Config;
use crate::markdown::{ContentType, WriteResult};
use chrono::{DateTime, Local};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub fn append_backlink(
    result: &WriteResult,
    note_date: DateTime<Local>,
    summary: Option<&str>,
    config: &Config,
) -> std::io::Result<Option<PathBuf>> {
    if !config.daily_notes.enabled {
        return Ok(None);
    }

    let note_dir = &config.daily_notes.path;
    fs::create_dir_all(note_dir)?;

    let note_path = note_dir.join(format!("{}.md", note_date.format("%Y-%m-%d")));
    let section = match result.content_type {
        ContentType::Meeting => "Meetings",
        ContentType::Memo => "Voice Memos",
        ContentType::Dictation => "Dictations",
    };
    let link_target = relative_or_absolute_link(note_dir, &result.path);
    let bullet = if let Some(excerpt) = summary_excerpt(summary) {
        format!("- [{}]({}) — {}", result.title, link_target, excerpt)
    } else {
        format!("- [{}]({})", result.title, link_target)
    };

    let mut content = if note_path.exists() {
        fs::read_to_string(&note_path)?
    } else {
        format!("# {}\n\n", note_date.format("%Y-%m-%d"))
    };

    if content.contains(&format!("]({})", link_target)) {
        return Ok(Some(note_path));
    }

    if let Some(index) = content.find(&format!("## {}\n", section)) {
        let insert_at = section_insert_position(&content[index..]).map(|offset| index + offset);
        let position = insert_at.unwrap_or(content.len());
        if position > 0 && !content[..position].ends_with('\n') {
            content.insert(position, '\n');
        }
        content.insert_str(position, &format!("{}\n", bullet));
    } else {
        if !content.ends_with("\n\n") {
            if !content.ends_with('\n') {
                content.push('\n');
            }
            content.push('\n');
        }
        content.push_str(&format!("## {}\n\n{}\n", section, bullet));
    }

    fs::write(&note_path, content)?;
    Ok(Some(note_path))
}

fn section_insert_position(section_text: &str) -> Option<usize> {
    let body = section_text.find('\n').map(|idx| idx + 1)?;
    let remainder = &section_text[body..];
    remainder.find("\n## ").map(|idx| body + idx)
}

fn summary_excerpt(summary: Option<&str>) -> Option<String> {
    let summary = summary?;
    let line = summary
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("## "))?;
    let cleaned = line.trim_start_matches("- ").trim();
    if cleaned.is_empty() {
        return None;
    }

    let excerpt: String = cleaned.chars().take(140).collect();
    if cleaned.chars().count() > 140 {
        Some(format!("{}...", excerpt))
    } else {
        Some(excerpt)
    }
}

fn relative_or_absolute_link(from_dir: &Path, target: &Path) -> String {
    relative_path(from_dir, target)
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| target.display().to_string())
}

fn relative_path(from_dir: &Path, target: &Path) -> Option<PathBuf> {
    let from = fs::canonicalize(from_dir).ok()?;
    let to = fs::canonicalize(target).ok()?;

    let from_components: Vec<_> = from.components().collect();
    let to_components: Vec<_> = to.components().collect();

    let mut common = 0usize;
    while common < from_components.len()
        && common < to_components.len()
        && from_components[common] == to_components[common]
    {
        common += 1;
    }

    let mut relative = PathBuf::new();
    for component in &from_components[common..] {
        if matches!(component, Component::Normal(_)) {
            relative.push("..");
        }
    }
    for component in &to_components[common..] {
        relative.push(component.as_os_str());
    }

    Some(relative)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn write_result(path: PathBuf, title: &str, content_type: ContentType) -> WriteResult {
        WriteResult {
            path,
            title: title.to_string(),
            word_count: 10,
            content_type,
        }
    }

    #[test]
    fn append_backlink_creates_daily_note_sections() {
        let dir = TempDir::new().unwrap();
        let meetings_dir = dir.path().join("meetings");
        let daily_dir = dir.path().join("daily");
        fs::create_dir_all(&meetings_dir).unwrap();
        let meeting_path = meetings_dir.join("2026-03-19-pricing-review.md");
        fs::write(&meeting_path, "# Pricing Review\n").unwrap();

        let mut config = Config::default();
        config.output_dir = meetings_dir.clone();
        config.daily_notes.enabled = true;
        config.daily_notes.path = daily_dir.clone();

        let result = write_result(meeting_path, "Pricing Review", ContentType::Meeting);
        let note_path = append_backlink(
            &result,
            Local.with_ymd_and_hms(2026, 3, 19, 9, 0, 0).unwrap(),
            Some("## Summary\n\n- Locked pricing at monthly billing.\n"),
            &config,
        )
        .unwrap()
        .unwrap();

        let note = fs::read_to_string(note_path).unwrap();
        assert!(note.contains("# 2026-03-19"));
        assert!(note.contains("## Meetings"));
        assert!(note.contains("[Pricing Review]("));
        assert!(note.contains("Locked pricing at monthly billing."));
    }

    #[test]
    fn append_backlink_is_idempotent_for_same_artifact() {
        let dir = TempDir::new().unwrap();
        let meetings_dir = dir.path().join("meetings");
        let daily_dir = dir.path().join("daily");
        fs::create_dir_all(&meetings_dir).unwrap();
        let memo_path = meetings_dir.join("memos").join("2026-03-19-onboarding.md");
        fs::create_dir_all(memo_path.parent().unwrap()).unwrap();
        fs::write(&memo_path, "# Onboarding Idea\n").unwrap();

        let mut config = Config::default();
        config.output_dir = meetings_dir.clone();
        config.daily_notes.enabled = true;
        config.daily_notes.path = daily_dir.clone();

        let result = write_result(memo_path, "Onboarding Idea", ContentType::Memo);
        let date = Local.with_ymd_and_hms(2026, 3, 19, 9, 0, 0).unwrap();

        append_backlink(&result, date, Some("Short memo summary"), &config).unwrap();
        append_backlink(&result, date, Some("Short memo summary"), &config).unwrap();

        let note = fs::read_to_string(daily_dir.join("2026-03-19.md")).unwrap();
        assert_eq!(note.matches("[Onboarding Idea](").count(), 1);
        assert!(note.contains("## Voice Memos"));
    }
}
