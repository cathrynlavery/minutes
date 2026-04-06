//! Fact extraction from meeting transcripts for knowledge base updates.
//!
//! Two-phase extraction with safety guardrails:
//! 1. **Structured-first** (no LLM): extract from YAML frontmatter — decisions, action_items, entities.
//!    Zero hallucination risk since these are already LLM-validated during summarization.
//! 2. **LLM extraction** (optional, engine != "none"): mine transcript body for richer facts.
//!    Conservative prompt: "only extract explicitly stated facts, never infer."

use crate::knowledge::{slugify, Confidence, Fact, PersonFacts};
use crate::markdown::Frontmatter;

/// Extract facts from structured frontmatter only (phase 1 — zero hallucination risk).
/// This is the default mode when `[knowledge] engine = "none"`.
pub fn extract_from_frontmatter(fm: &Frontmatter, meeting_path: &str) -> Vec<PersonFacts> {
    let date = fm.date.format("%Y-%m-%d").to_string();
    let meeting_slug = meeting_path
        .rsplit('/')
        .next()
        .unwrap_or(meeting_path)
        .trim_end_matches(".md")
        .to_string();

    let mut person_map: std::collections::HashMap<String, PersonFacts> =
        std::collections::HashMap::new();

    // Extract from action_items (high-value: explicit assignee + task)
    for item in &fm.action_items {
        if item.status == "done" {
            continue;
        }
        let slug = slugify(&item.assignee);
        if slug.is_empty() {
            continue;
        }
        let entry = person_map
            .entry(slug.clone())
            .or_insert_with(|| PersonFacts {
                slug: slug.clone(),
                name: item.assignee.clone(),
                facts: vec![],
            });
        entry.facts.push(Fact {
            text: format!(
                "Committed to: {} (due: {})",
                item.task,
                item.due.as_deref().unwrap_or("unset")
            ),
            category: "commitment".into(),
            confidence: Confidence::Explicit,
            source_meeting: meeting_slug.clone(),
            source_date: date.clone(),
        });
    }

    // Extract from decisions (linked to topic, attributed to all attendees)
    for decision in &fm.decisions {
        // Decisions are attributed to the meeting, not a specific person.
        // We file them under each attendee present.
        for attendee in &fm.attendees {
            let slug = slugify(attendee);
            if slug.is_empty() {
                continue;
            }
            let entry = person_map
                .entry(slug.clone())
                .or_insert_with(|| PersonFacts {
                    slug: slug.clone(),
                    name: attendee.clone(),
                    facts: vec![],
                });
            let topic_str = decision
                .topic
                .as_deref()
                .map(|t| format!(" [{}]", t))
                .unwrap_or_default();
            entry.facts.push(Fact {
                text: format!("Decision{}: {}", topic_str, decision.text),
                category: "decision".into(),
                confidence: Confidence::Strong,
                source_meeting: meeting_slug.clone(),
                source_date: date.clone(),
            });
        }
    }

    // Extract from entities.people (presence facts — they were in this meeting)
    for entity in &fm.entities.people {
        let slug = slugify(&entity.slug);
        if slug.is_empty() {
            continue;
        }
        // Only create the entry if they don't already have facts from above.
        // Avoids cluttering with "was in meeting" for people we already have richer data on.
        if !person_map.contains_key(&slug) {
            person_map.insert(
                slug.clone(),
                PersonFacts {
                    slug: slug.clone(),
                    name: entity.label.clone(),
                    facts: vec![Fact {
                        text: format!("Attended meeting: {}", fm.title),
                        category: "context".into(),
                        confidence: Confidence::Strong,
                        source_meeting: meeting_slug.clone(),
                        source_date: date.clone(),
                    }],
                },
            );
        }
    }

    // Extract from intents
    for intent in &fm.intents {
        if let Some(ref who) = intent.who {
            let slug = slugify(who);
            if slug.is_empty() {
                continue;
            }
            let entry = person_map
                .entry(slug.clone())
                .or_insert_with(|| PersonFacts {
                    slug: slug.clone(),
                    name: who.clone(),
                    facts: vec![],
                });
            let kind_label = format!("{:?}", intent.kind)
                .to_lowercase()
                .replace("_", " ");
            entry.facts.push(Fact {
                text: format!("{}: {}", capitalize_first(&kind_label), intent.what),
                category: match intent.kind {
                    crate::markdown::IntentKind::ActionItem
                    | crate::markdown::IntentKind::Commitment => "commitment".into(),
                    crate::markdown::IntentKind::Decision => "decision".into(),
                    crate::markdown::IntentKind::OpenQuestion => "context".into(),
                },
                confidence: Confidence::Strong,
                source_meeting: meeting_slug.clone(),
                source_date: date.clone(),
            });
        }
    }

    person_map.into_values().collect()
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::{ActionItem, ContentType, Decision, EntityLinks, EntityRef, Frontmatter};
    use chrono::{Local, TimeZone};

    fn test_frontmatter() -> Frontmatter {
        Frontmatter {
            title: "Q2 Strategy Call".into(),
            r#type: ContentType::Meeting,
            date: Local.with_ymd_and_hms(2026, 4, 3, 14, 0, 0).unwrap(),
            duration: "30m".into(),
            source: None,
            status: None,
            tags: vec![],
            attendees: vec!["Mat".into(), "Dan".into()],
            calendar_event: None,
            people: vec![],
            entities: EntityLinks {
                people: vec![
                    EntityRef {
                        slug: "mat".into(),
                        label: "Mat".into(),
                        aliases: vec![],
                    },
                    EntityRef {
                        slug: "dan-benamoz".into(),
                        label: "Dan Benamoz".into(),
                        aliases: vec![],
                    },
                ],
                projects: vec![],
            },
            device: None,
            captured_at: None,
            context: None,
            action_items: vec![
                ActionItem {
                    assignee: "Mat".into(),
                    task: "Send pricing doc to Dan".into(),
                    due: Some("2026-04-05".into()),
                    status: "open".into(),
                },
                ActionItem {
                    assignee: "Dan".into(),
                    task: "Review compliance requirements".into(),
                    due: None,
                    status: "open".into(),
                },
            ],
            decisions: vec![Decision {
                text: "Switch to monthly billing for pharmacy consultations".into(),
                topic: Some("pricing".into()),
            }],
            intents: vec![],
            recorded_by: None,
            visibility: None,
            speaker_map: vec![],
            filter_diagnosis: None,
        }
    }

    #[test]
    fn extracts_action_items_as_commitments() {
        let fm = test_frontmatter();
        let results = extract_from_frontmatter(&fm, "2026-04-03-strategy.md");

        let mat_facts: Vec<&PersonFacts> = results.iter().filter(|pf| pf.slug == "mat").collect();
        assert_eq!(mat_facts.len(), 1);

        let commitment = mat_facts[0]
            .facts
            .iter()
            .find(|f| f.category == "commitment")
            .expect("should have commitment fact");
        assert!(commitment.text.contains("Send pricing doc"));
        assert_eq!(commitment.confidence, Confidence::Explicit);
        assert_eq!(commitment.source_meeting, "2026-04-03-strategy");
    }

    #[test]
    fn extracts_decisions_for_each_attendee() {
        let fm = test_frontmatter();
        let results = extract_from_frontmatter(&fm, "2026-04-03-strategy.md");

        // Both Mat and Dan should get the pricing decision
        for name in &["mat", "dan"] {
            let pf: Vec<&PersonFacts> = results.iter().filter(|pf| pf.slug == *name).collect();
            assert!(!pf.is_empty(), "{} should have facts", name);
            let has_decision = pf[0].facts.iter().any(|f| f.category == "decision");
            assert!(has_decision, "{} should have decision fact", name);
        }
    }

    #[test]
    fn skips_done_action_items() {
        let mut fm = test_frontmatter();
        fm.action_items = vec![ActionItem {
            assignee: "Alice".into(),
            task: "Already completed".into(),
            due: None,
            status: "done".into(),
        }];
        fm.decisions = vec![];
        fm.entities.people = vec![];
        fm.attendees = vec![];

        let results = extract_from_frontmatter(&fm, "test.md");
        assert!(results.is_empty(), "done items should not produce facts");
    }

    #[test]
    fn entity_presence_only_when_no_richer_facts() {
        let mut fm = test_frontmatter();
        // Dan has action items + decisions (richer facts)
        // Add a third entity who has no action items or decisions
        fm.entities.people.push(EntityRef {
            slug: "jex-musa".into(),
            label: "Jex Musa".into(),
            aliases: vec![],
        });

        let results = extract_from_frontmatter(&fm, "test.md");
        let jex: Vec<&PersonFacts> = results.iter().filter(|pf| pf.slug == "jex-musa").collect();
        assert_eq!(jex.len(), 1);
        assert_eq!(jex[0].facts.len(), 1);
        assert!(jex[0].facts[0].text.contains("Attended meeting"));
    }
}
