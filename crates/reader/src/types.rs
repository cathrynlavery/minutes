use chrono::{DateTime, Local};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ──────────────────────────────────────────────────────────────
// Standalone types for parsing Minutes meeting files.
// Duplicated from minutes-core to avoid pulling in audio deps.
// ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    Meeting,
    Memo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum OutputStatus {
    Complete,
    NoSpeech,
    TranscriptOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Frontmatter {
    pub title: String,
    pub r#type: ContentType,
    pub date: DateTime<Local>,
    pub duration: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<OutputStatus>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attendees: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calendar_event: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub people: Vec<String>,
    #[serde(default, skip_serializing_if = "EntityLinks::is_empty")]
    pub entities: EntityLinks,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_items: Vec<ActionItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<Decision>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub intents: Vec<Intent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recorded_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<Visibility>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub speaker_map: Vec<SpeakerAttribution>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AttributionSource {
    Deterministic,
    Llm,
    Enrollment,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SpeakerAttribution {
    pub speaker_label: String,
    pub name: String,
    pub confidence: Confidence,
    pub source: AttributionSource,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct EntityLinks {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub people: Vec<EntityRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projects: Vec<EntityRef>,
}

impl EntityLinks {
    pub fn is_empty(&self) -> bool {
        self.people.is_empty() && self.projects.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EntityRef {
    pub slug: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActionItem {
    pub assignee: String,
    pub task: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Decision {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum IntentKind {
    ActionItem,
    Decision,
    OpenQuestion,
    Commitment,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Intent {
    pub kind: IntentKind,
    pub what: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub who: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by_date: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Private,
    Team,
}

#[derive(Debug, Clone)]
pub struct ParsedMeeting {
    pub frontmatter: Frontmatter,
    pub body: String,
    pub path: PathBuf,
}
