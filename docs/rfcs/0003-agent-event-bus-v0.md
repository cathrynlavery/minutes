# RFC 0003: Agent Event Bus v0 Contract

Status: accepted as the v0 contract baseline for GitHub #194.

This document freezes the current Agent Event Bus contract before more event
emitters are added. The goal is to keep the shipped CLI/MCP bus compatible
while making the remaining #194 work explicit.

## Current Shipped Baseline

Minutes already ships the durable bus plumbing:

- append-only JSONL at `~/.minutes/events.jsonl`
- monotonic `seq` assigned under the writer lock
- `events.seq` sidecar cursor for O(1) normal appends
- `minutes events --follow --since-seq N`
- MCP `minutes://events/live` and `minutes://events/live{?since_seq,limit}`
- MCP `resources/subscribe` with `notifications/resources/updated`
- `live.utterance.final` emission from the live transcript writer

## Wire Envelope

The v0 wire format is a flat JSON object, not a nested event object:

```jsonc
{
  "v": 1,
  "seq": 4826,
  "timestamp": "2026-04-29T08:30:00-07:00",
  "event_type": "live.utterance.final",
  "... event-specific fields ...": "..."
}
```

This shape is the canonical v0 persistence and streaming contract for CLI and
MCP consumers.

Compatibility rules:

- `v` defaults to `1` when absent so older log lines still read.
- `seq` defaults to `0` when absent and is repaired while reading legacy logs.
- `timestamp` remains the v0 field name. Do not introduce a top-level `ts`
  synonym in v0.
- `event_type` is the event discriminator.
- Event-specific payload fields remain flattened at top level.
- Do not add a top-level `event`, `kind`, `source`, `confidence`, or
  `provenance` envelope field in v0. Those concepts belong inside typed event
  payloads when needed.

Why not migrate to the original #194 top-level envelope immediately?

The current flat shape is already shipped through the CLI and MCP live resource.
Changing the persisted JSONL shape now would force a compatibility layer before
the remaining emitters are even implemented. v0 therefore freezes the shipped
shape and reserves a future v2 envelope for a cleaner nested/projection model if
real subscribers prove that need.

## Event Taxonomy

The v0 taxonomy has two buckets: shipped legacy names that must keep reading,
and dotted agent-facing names used by new or normalized events.

| Event | v0 status | Notes |
| --- | --- | --- |
| `live.utterance.final` | canonical, shipped | Emitted by live transcript writer. Legacy `LiveUtteranceFinal` is accepted as an alias. |
| `recording.completed` | canonical, shipped | Emitted by recording completion paths. Legacy `RecordingCompleted` is accepted as an alias. |
| `meeting.insight.detected` | canonical alias, compatibility bridge | Existing emitters still serialize `MeetingInsightExtracted`; readers accept `meeting.insight.detected`. `minutes-l5sa.3` owns normalizing semantics. |
| `recording.started` | canonical, shipped | Emitted after capture/live/dictation/watch processing actually starts. |
| `transcript.delta` | undecided for v0 | Owned by `minutes-l5sa.3`. Either implement behind an explicit gate or formally punt from v0. |
| `agent.annotation` | planned | Owned by `minutes-l5sa.4`. Must be append-only, attributed, allowlisted, and separate from human-authored notes. |

Existing internal or legacy events such as `AudioProcessed`, `WatchProcessed`,
`NoteAdded`, `VaultSynced`, `VoiceMemoProcessed`, `DeviceChanged`,
`KnowledgeUpdated`, `MicMuted`, and `MicUnmuted` remain valid log entries.
They are not part of the #194 v0 agent contract unless a later bead promotes
them into the dotted taxonomy.

## Closure Rules For #194

GitHub #194 should not close merely because the bus plumbing exists. It can
close only after:

1. This envelope/taxonomy contract is tested and referenced from the issue.
2. Recording lifecycle events are implemented or explicitly scoped.
3. Live transcript delta and semantic insight behavior is settled.
4. Agent annotation write-back is implemented with an allowlist or split out
   with a clear rationale.
5. MCP subscription behavior is tested against real hosts where available.
6. An adversarial review finds no remaining P1/P2 contract gaps.
