use crate::config::Config;
use crate::error::TranscribeError;
use crate::markdown::ContentType;
use crate::transcribe::{self, TranscribeResult};
use std::path::{Path, PathBuf};

use whisper_guard::segments as wg_segments;

#[derive(Debug, Clone)]
pub struct TranscriptionRequest {
    pub audio_path: PathBuf,
    pub content_type: ContentType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TranscriptCleanupStage {
    DedupSegments,
    DedupInterleaved,
    StripForeignScript,
    CollapseNoiseMarkers,
    TrimTrailingNoise,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TranscriptCleanupStageStat {
    pub stage: TranscriptCleanupStage,
    pub before: usize,
    pub after: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct TranscriptCleanupResult {
    pub lines: Vec<String>,
    pub stats: Vec<TranscriptCleanupStageStat>,
}

type TranscriptCleanupFn = fn(Vec<String>) -> Vec<String>;
type TranscriptCleanupStep = (TranscriptCleanupStage, TranscriptCleanupFn);

pub(crate) fn dedup_segments(lines: Vec<String>) -> Vec<String> {
    wg_segments::dedup_segments(&lines)
}

pub(crate) fn dedup_interleaved(lines: Vec<String>) -> Vec<String> {
    wg_segments::dedup_interleaved(&lines)
}

pub(crate) fn trim_trailing_noise(lines: Vec<String>) -> Vec<String> {
    wg_segments::trim_trailing_noise(&lines)
}

pub(crate) fn strip_foreign_script(lines: Vec<String>) -> Vec<String> {
    wg_segments::strip_foreign_script(&lines)
}

pub(crate) fn collapse_noise_markers(lines: Vec<String>) -> Vec<String> {
    wg_segments::collapse_noise_markers(&lines)
}

impl TranscriptCleanupResult {
    pub(crate) fn after(&self, stage: TranscriptCleanupStage) -> usize {
        self.stats
            .iter()
            .find(|stat| stat.stage == stage)
            .map(|stat| stat.after)
            .unwrap_or(self.lines.len())
    }
}

pub(crate) fn run_transcript_cleanup_pipeline(lines: Vec<String>) -> TranscriptCleanupResult {
    let mut stats = Vec::new();
    let mut current = lines;

    let stages: &[TranscriptCleanupStep] = &[
        (TranscriptCleanupStage::DedupSegments, dedup_segments),
        (TranscriptCleanupStage::DedupInterleaved, dedup_interleaved),
        (
            TranscriptCleanupStage::StripForeignScript,
            strip_foreign_script,
        ),
        (
            TranscriptCleanupStage::CollapseNoiseMarkers,
            collapse_noise_markers,
        ),
        (
            TranscriptCleanupStage::TrimTrailingNoise,
            trim_trailing_noise,
        ),
    ];

    for (stage, apply) in stages {
        let before = current.len();
        current = apply(current);
        stats.push(TranscriptCleanupStageStat {
            stage: *stage,
            before,
            after: current.len(),
        });
    }

    TranscriptCleanupResult {
        lines: current,
        stats,
    }
}

pub fn transcribe_request(
    request: &TranscriptionRequest,
    config: &Config,
) -> Result<TranscribeResult, TranscribeError> {
    match request.content_type {
        ContentType::Meeting => transcribe::transcribe_meeting(&request.audio_path, config),
        _ => transcribe::transcribe(&request.audio_path, config),
    }
}

pub fn transcribe_path_for_content(
    audio_path: &Path,
    content_type: ContentType,
    config: &Config,
) -> Result<TranscribeResult, TranscribeError> {
    let request = TranscriptionRequest {
        audio_path: audio_path.to_path_buf(),
        content_type,
    };
    transcribe_request(&request, config)
}
