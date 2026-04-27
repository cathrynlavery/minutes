# RFC 0001: First-class templates for domain-shaped summaries

- **Status**: Draft
- **Authors**: @silverstein, @ed0c
- **Related**: #143
- **Created**: 2026-04-17

## Summary

Introduce **templates** as a first-class primitive in Minutes. A template is a markdown file with YAML frontmatter that extends Minutes' structured extraction for a specific domain (medical notes, standup summaries, sales discovery, legal intake, etc.). Templates are additive, not replacement: they preserve the baseline extraction contract (`KEY POINTS`, `DECISIONS`, `ACTION ITEMS`, `OPEN QUESTIONS`, `COMMITMENTS`, `PARTICIPANTS`) and layer custom extraction fields, agent context, compliance rules, and additional prompt instructions on top.

Templates ship bundled with Minutes, live in `~/.minutes/templates/` for user customization, and get contributed to a `templates/` directory in this repository as a community library.

## Motivation

Issue #143 surfaced a real limitation: the summarization prompt is hardcoded in `crates/core/src/summarize.rs`, which forces a recompile for any customization and offers no path to domain-specific output formats.

The straightforward reading of that ask is "add `--prompt` / `--prompt-file` flags." That approach breaks the pipeline: the current prompt isn't a casual default, it's the contract that produces the structured output parsed into YAML frontmatter. That frontmatter powers `minutes search-actions`, the MCP tools, the knowledge graph, hooks, skills, and the agent coaching loop. A replacement prompt would silently break all of that without an obvious error signal.

Templates solve the underlying problem (domain customization without recompile) while preserving the structured-extraction contract, and they unlock three things a prompt flag cannot:

1. **Vertical product surfaces**: per-template landing pages, SEO-indexable, each with a working example
2. **Community contribution**: medical, legal, therapy, and sales verticals need domain expertise Minutes will never have internally; templates let that expertise live in community-PRable markdown files
3. **Agent-aware domain semantics**: templates carry `agent_context` that informs Claude/Codex/Gemini/OpenCode when working with template-tagged meetings, without prompting users to re-explain context

## Non-goals

- Replace or bypass the baseline structured extraction
- Support arbitrary free-form prompts that produce unparseable output
- Build a separate package registry before a repo-based contribution workflow exists
- Duplicate skill functionality (templates define *schema*; skills define *interaction*)

## Prior art

- **Granola templates**: closed-source, opaque dropdown, single-vendor. The closest functional analog, and their moat. Minutes can do better by being markdown-native and community-driven.
- **Fathom AI Summary Templates**: similar dropdown UX
- **Fireflies SmartMatch**: keyword-triggered templates
- **Obsidian templates**: variable substitution, no LLM awareness
- **Rust RFCs / Python PEPs**: the RFC process itself; structured proposals with implementation phases

## Design

### Three-layer architecture this clarifies

- **Capture layer**: audio → transcript (Rust pipeline, whisper/parakeet, diarization). Unchanged.
- **Schema layer**: transcript → structured extraction, **template-aware**. This RFC defines this layer.
- **Interaction layer**: structured data → agent conversation via skills, MCP, hooks, graph. Templates can route to skills but don't replace them.

Templates and skills compose. A SOAP template guarantees the `subjective`/`objective`/`assessment`/`plan` shape in frontmatter; a `/minutes-soap-review` skill knows how to walk an agent through that shape conversationally. Template = schema; skill = interaction.

### Template anatomy

A template is a markdown file:

```yaml
---
name: Engineering Standup
slug: standup
version: 1.0.0
author: silverstein
license: MIT
description: Engineering standup summary with yesterday/today/blockers.
keywords: [standup, daily, engineering]
extends_base: true
triggers:
  calendar_keywords: [standup, daily, scrum]
  transcript_keywords: [yesterday, today, blocked]
extract:
  yesterday: "What was completed since the last standup"
  today: "Plans for today"
  blockers:
    technical: "Engineering blockers"
    cross_team: "Cross-team dependencies"
post_record_skill: minutes-standup-digest
agent_context: |
  This is an engineering standup. Keep responses engineering-specific.
additional_instructions: |
  Be concise. Blockers are the priority section.
language: en
---

# Engineering Standup Template

Human-readable documentation goes here: usage notes, examples, edge cases.
```

### Frontmatter fields

| Field | Type | Purpose |
|---|---|---|
| `name` | string | Human-readable name |
| `slug` | string | CLI identifier, URL-safe |
| `version` | semver | Template versioning for upgrades |
| `author` | string | Contributor credit |
| `license` | string | Per-template license (default MIT) |
| `description` | string | One-line summary (for listing + SEO) |
| `keywords` | [string] | Search + SEO |
| `extends` | slug (optional) | Inherit from another template (see Inheritance) |
| `extends_base` | bool | If true, baseline structured extraction still runs and custom fields layer on top. Default true. |
| `triggers.calendar_keywords` | [string] | Auto-select template from calendar event title |
| `triggers.transcript_keywords` | [string] | Post-hoc suggestion if no template was picked |
| `extract` | object | Custom extraction schema. Values can be strings (descriptions) or nested objects (sub-fields). Capped at 3 levels. |
| `post_record_skill` | string | `/minutes-*` skill invoked by post-record hook |
| `agent_context` | string | Injected into LLM prompts for agents working with this meeting later |
| `compliance` | object | Declarative rules (see Compliance) |
| `additional_instructions` | string | Appended to base system prompt (NEVER replaces) |
| `language` | string | Override `[summarization] language` for this template |

### Storage and resolution

Templates are resolved in this order (earlier wins):

1. Project: `.minutes/templates/` (repo-local, checked into git)
2. User: `~/.minutes/templates/`
3. Bundled: shipped with the binary (`crates/assets/templates/`)
4. Community: `templates/` in this repository, distributed with releases

Users can override bundled templates by putting a file with the same slug in `~/.minutes/templates/`.

### Inheritance (family pattern)

Templates can extend another template via `extends:`. Common use case: shared compliance + language + agent_context across a family of domain-specific templates.

```yaml
# medical-fr-base.md
---
name: Medical (French, base)
slug: medical-fr-base
language: fr
compliance:
  redact_phi: true
  require_local_processing: true
agent_context: |
  This is clinical data under French HDS and RGPD.
additional_instructions: |
  Utilisez la terminologie clinique française.
---
```

```yaml
# consultation-fr.md
---
name: Consultation Report (French)
slug: consultation-fr
extends: medical-fr-base
extract:
  motif_consultation: "Motif de la consultation"
  anamnese: "Anamnèse"
  examen_clinique: "Résultats de l'examen clinique"
  assessment:
    diagnostic_principal: "Diagnostic principal"
    diagnostics_differentiels: "Diagnostics différentiels"
  plan:
    traitement: "Traitement prescrit"
    suivi: "Suivi recommandé"
---
```

Inheritance rules:
- Child inherits: `compliance`, `agent_context`, `additional_instructions`, `language`
- Child overrides: `extract`, `triggers`, `post_record_skill`, `name`, `description`, `keywords`
- Child merges (with conflict warning): `compliance` individual fields, `additional_instructions` (concat)
- Inheritance is single-parent (no multiple inheritance); grandparents resolve transitively

### Extract fields

`extract:` accepts either a string (a description for a flat field) or a nested object (sub-fields, each with its own string or nested-object value). Nesting is capped at 3 levels.

```yaml
extract:
  subjective: "Patient-reported symptoms and history"          # flat
  objective: "Exam findings, vitals, labs"
  assessment:                                                   # nested
    diagnosis: "Primary clinical impression"
    differential: "Differential diagnoses"
  plan:
    treatment:                                                  # 3rd level
      medications: "Prescribed medications with dosing"
      procedures: "Procedures performed"
    followup: "Next steps and referrals"
```

The summarizer converts the `extract:` tree into a JSON schema, passes it to the LLM as structured-output guidance, and round-trips the result back into YAML frontmatter with the nested shape intact.

Reliability note: depth > 3 levels is unreliable with current open-weights models and will produce a validation error at template-load time.

### Triggers and auto-selection

If `--template` isn't explicitly provided on the command line, Minutes picks a template in this order:

1. Match `triggers.calendar_keywords` against the upcoming/current calendar event title (requires calendar integration enabled)
2. After transcription, match `triggers.transcript_keywords` against the transcript content
3. Fall back to the `meeting` template

Manual override: `minutes record --template <slug>` or `minutes process <file> --template <slug>`.

### Compliance

The `compliance` field encodes declarative rules checked by the pipeline:

| Field | Type | Behavior |
|---|---|---|
| `redact_phi` | bool | Post-extraction, redact likely PHI patterns (names, DOBs, phone numbers, MRNs) before persistence |
| `forbid_in_summary` | [string] | Enum of `[phone_number, full_ssn, full_dob, full_name, email, mrn, credit_card_number, bank_account_number, auth_secret, dea_number]`; validator rejects summary if detected. The last four are non-clinical high-risk identifiers (financial, credentials, prescriber registration) that should not surface in any summary regardless of template purpose. |
| `require_local_processing` | bool | If true, Minutes errors when a cloud summarization engine (Claude, OpenAI, Mistral) is configured |
| `retention_days` | int | Annotates frontmatter with `retention_until`; downstream tools can enforce deletion |
| `audit_log` | bool | Writes a timestamped entry to `~/.minutes/logs/audit.log` (template, action, file hash) |

Compliance rules compose through inheritance. Child can tighten (stricter) but a warning fires if child loosens a parent's rule.

### CLI surface

```bash
minutes template list                         # list installed templates
minutes template show <slug>                  # dump template contents
minutes template install <url|gh:user/repo>   # install from URL / gh repo / gist
minutes template search <query>               # search gallery + installed
minutes template create <name>                # scaffold new template with heredoc
minutes template validate <path>              # schema check + smoke test
minutes template upgrade                      # check for template updates

minutes record --template <slug>              # record with explicit template
minutes process <file> --template <slug>      # re-process existing recording with new template
```

### Agent integration

When an agent (Claude Desktop, Code, Cowork, Codex, Gemini, OpenCode) interacts with a template-tagged meeting via MCP:

- The meeting's frontmatter includes `template: <slug>` and the full extracted shape
- MCP tool responses include `agent_context` from the template, injected as guidance
- Skills declared via `post_record_skill` are invoked automatically on record completion

The interaction layer stays skill-driven; templates just enrich what skills have to work with.

## Worked examples

### `meeting` (bundled, baseline)

Minimal default, used when no other template matches.

```yaml
---
name: Meeting
slug: meeting
version: 1.0.0
description: Generic meeting summary (default).
extends_base: true
---
```

### `standup` (bundled)

```yaml
---
name: Engineering Standup
slug: standup
extends_base: true
triggers:
  calendar_keywords: [standup, daily, scrum]
extract:
  yesterday: "What was completed since last standup"
  today: "Plans for today"
  blockers:
    technical: "Engineering blockers"
    cross_team: "Cross-team dependencies"
post_record_skill: minutes-standup-digest
---
```

### `medical-fr-base` (community, co-authored with @ed0c)

See Inheritance section above.

### `consultation-fr` (community, extends `medical-fr-base`)

See Inheritance section above.

### `soap-fr` (community, extends `medical-fr-base`)

```yaml
---
name: SOAP (French)
slug: soap-fr
extends: medical-fr-base
description: Note SOAP en français pour consultations médicales
extract:
  subjective: "Symptômes rapportés par le patient"
  objective: "Examen clinique, signes vitaux, résultats de laboratoire"
  assessment:
    diagnostic: "Diagnostic principal"
    differentiel: "Diagnostics différentiels"
  plan:
    traitement: "Traitement"
    suivi: "Suivi et orientations"
post_record_skill: minutes-soap-review
---
```

### US clinical family (`soap` + `psych-soap` + `pediatric-soap`)

A second clinical family, parallel to `medical-fr-base`. The two families are deliberately independent, rather than sharing a `clinical-base` parent: US HIPAA obligations and French HDS/RGPD obligations do not share a clean common base yet, and forcing one would lock in lowest-common-denominator design before either family is mature. If convergent patterns emerge, a shared parent can be extracted later.

The `soap` parent encodes a widely used SOAP structure described in StatPearls/NCBI Bookshelf. Two specialty children pressure-test inheritance: `psych-soap` reorganizes Objective around the Mental Status Exam, and `pediatric-soap` introduces growth percentiles, developmental screening, and immunization fields that do not exist in the adult parent.

Compliance scope note: these templates are HIPAA-aware, not HIPAA-compliant in a regulated deployment. HIPAA compliance is ultimately a property of deployment (BAAs, infrastructure, access controls), not file format. Open Question 9 covers this explicitly. The bundled `soap` markdown body includes an explicit compliance scope statement: `audit_log: true` and `require_local_processing: true` are tool-level safety behaviors, not deployment-level certifications, and medical-record / audit-log retention are governed by state law and other applicable retention obligations.

#### `soap` (community, US clinical baseline)

```yaml
---
name: SOAP Note (US Clinical, Outpatient)
slug: soap
version: 1.0.0
author: silverstein
license: MIT
description: SOAP note for US outpatient clinical encounters. Inpatient progress notes have additional structure (interval events, I/O, lines, prophylaxis) and would be a separate child template.
keywords: [soap, medical, clinical, hipaa, us, outpatient]
extends_base: true
triggers:
  calendar_keywords: [patient, consultation, visit, "follow-up", appointment, encounter]
  transcript_keywords: ["chief complaint", vitals, assessment, plan]
extract:
  patient_context:
    age_sex: "Patient age and sex/gender if clinically relevant"
    visit_type: "New, follow-up, urgent, telehealth"
    historian: "Patient, parent, caregiver; reliability if relevant"
  subjective:
    chief_complaint: "Patient-reported reason for visit, in their own words"
    history_present_illness: "HPI using OLDCARTS (onset, location, duration, character, alleviating/aggravating, radiation, timing, severity), associated symptoms, and pertinent negatives"
    past_medical_history: "Active and past chronic conditions"
    past_surgical_history: "Prior surgeries with year when known"
    family_history: "Pertinent first-degree family conditions"
    social_history: "Occupation, living situation, tobacco/alcohol/substance use, sexual history (HEADSS framework in adolescents)"
    medications: "Current medications with reconciliation and adherence: name, dose, route, frequency"
    allergies: "Drug, food, environmental allergies with reaction and severity; NKDA if explicitly stated"
    review_of_systems: "Pertinent positives and negatives by organ system"
  objective:
    vitals: "BP, HR, RR, temperature, SpO2, weight, height, BMI"
    physical_exam:
      general: "General appearance, distress level, alertness"
      cardiovascular: "Rate, rhythm, murmurs, peripheral pulses"
      neurological: "Mental status, cranial nerves, motor, sensory, reflexes"
      # Additional systems (HEENT, respiratory, abdominal, MSK, skin) in the bundled template; abbreviated here.
    labs: "Relevant lab results with reference ranges"
    imaging: "Imaging studies and impressions"
  assessment:
    primary_diagnosis: "Most likely diagnosis with explicit clinical reasoning"
    differential_diagnoses: "Ranked differentials, including dangerous diagnoses even when less likely"
    problem_list: "Active problems ordered by clinical importance"
  plan:
    diagnostic: "Further workup ordered with rationale, indexed to the assessment problem"
    therapeutic: "Medications, procedures, non-pharmacologic interventions, indexed to the assessment problem"
    education: "Patient education, counseling, shared decision-making notes"
    return_precautions: "Symptoms or thresholds that should prompt earlier return or ED visit"
    referrals: "Specialist consults requested"
    follow_up: "Disposition and follow-up timing"
compliance:
  redact_phi: false
  forbid_in_summary: [full_ssn, credit_card_number, bank_account_number, auth_secret]
  require_local_processing: true
  audit_log: true
post_record_skill: minutes-soap-review
agent_context: |
  This is a clinical SOAP summary and contains protected health information. It is the working medical record for many clinical workflows, so patient identifiers (names, MRNs, dates of birth) are part of the content and may be referenced when discussing clinical care. SSNs, credit card numbers, bank account numbers, and authentication secrets should not appear in a clinical note; if any surface in the transcript, omit them from the summary.
additional_instructions: |
  Document pertinent positives AND pertinent negatives in ROS and physical exam. Order differentials most-to-least likely with explicit clinical reasoning for the leading diagnosis, and include dangerous diagnoses even when less likely. Plan items should reference the assessment problem they address.
language: en
---
```

The `soap` template ships **identified by default**. A clinical SOAP note is the working medical record for many clinicians (small practices, telehealth, outpatient settings without an integrated EHR), and stripping patient names, MRNs, or DOBs from the canonical clinical artifact would defeat the template's primary purpose. The privacy posture comes from `require_local_processing: true` (the audio and transcript stay on the user's machine; no cloud LLM ingestion) plus `audit_log: true` (local activity logging), not from redacting the content of the note itself. `forbid_in_summary` is the safety-net validator: it rejects any summary containing SSNs, credit card numbers, bank account numbers, or authentication secrets, since those should never appear in a clinical note regardless of intent.

Deployments with different needs can override:
- Research, teaching, or agent-only use cases that require de-identified output: set `redact_phi: true` and add the relevant clinical identifiers (`full_name`, `mrn`, `full_dob`, `phone_number`, `email`) to `forbid_in_summary`.
- Cloud-processing deployments under appropriate Business Associate Agreements: override `require_local_processing: false`. HIPAA itself permits cloud processing with a BAA; `require_local_processing` is a Minutes product/safety policy, not a HIPAA requirement.

#### `psych-soap` (community, extends `soap`)

Reorganizes Objective around the Mental Status Exam, using APA Practice Guidelines / StatPearls-aligned domains. Inherits `compliance`, `agent_context`, `additional_instructions`, `language` from `soap`.

```yaml
---
name: Psychiatric SOAP Note
slug: psych-soap
extends: soap
version: 1.0.0
description: Psychiatric SOAP note with Mental Status Exam and DSM-5-TR diagnostic framework.
keywords: [psychiatry, "mental health", "mental status exam", "dsm-5-tr", soap]
triggers:
  calendar_keywords: [psych, psychiatry, therapy, counseling, "mental health"]
  transcript_keywords: [mood, affect, "mental status", "suicidal ideation", hallucinations]
extract:
  patient_context:
    age_sex: "Patient age and sex/gender if clinically relevant"
    visit_type: "Initial evaluation, medication management, follow-up, crisis"
    historian: "Patient, family member, prior records; reliability"
  subjective:
    chief_complaint: "Patient-reported reason for visit, in their own words"
    history_present_illness: "Mood, anxiety, sleep, appetite, energy, concentration, suicidal/homicidal ideation, psychotic symptoms, stressors, course since last visit"
    psychiatric_history: "Prior psychiatric diagnoses, hospitalizations, treatments, medication trials and responses"
    substance_use: "Alcohol, tobacco, cannabis, other substances; frequency and amount"
    trauma_history: "History of abuse, neglect, PTSD-related events"
    family_psychiatric_history: "Mental illness, suicide, substance use in family"
    social_history: "Living situation, supports, employment, legal involvement"
    medications: "Current psychiatric and medical medications with adherence"
    allergies: "Drug allergies and adverse reactions"
  objective:
    vitals: "BP, HR, weight, BMI for atypical antipsychotic monitoring"
    mental_status_exam:
      appearance: "Grooming, hygiene, dress, apparent age"
      behavior: "Eye contact, cooperativeness, attitude toward examiner"
      motor_activity: "Psychomotor agitation or retardation, abnormal movements (EPS, tardive dyskinesia, tremor)"
      speech: "Rate, rhythm, volume, prosody, latency"
      mood_affect: "Mood (patient-reported emotional state) and affect (observed range, intensity, congruence with mood)"
      thought_process: "Linear, tangential, circumstantial, flight of ideas, loose associations"
      thought_content: "Suicidal ideation, homicidal ideation, delusions, obsessions, paranoia"
      perceptual_disturbances: "Hallucinations (auditory, visual, tactile), illusions, depersonalization or derealization"
      sensorium_and_cognition: "Level of consciousness/alertness, orientation, attention, memory, fund of knowledge, abstract thinking"
      insight: "Awareness of illness and need for treatment"
      judgment: "Practical decision-making and ability to use information in current context"
    labs: "Medication levels (lithium, valproate, other AEDs); metabolic monitoring on antipsychotics (fasting glucose or A1c, lipid panel, basic metabolic panel); TSH, B12, drug screen if indicated"
  assessment:
    dsm5tr_diagnoses: "Active DSM-5-TR diagnoses with specifiers"
    differential_diagnoses: "Ranked differentials including medical mimics"
    risk_assessment: "Suicide risk, violence risk, self-care capacity"
    formulation: "Biopsychosocial formulation linking history to current presentation"
  plan:
    medication_management: "Psychiatric medications: starts, stops, dose changes, rationale, side-effect monitoring"
    psychotherapy: "Therapy modality, frequency, focus areas"
    safety_planning: "Safety plan, means restriction, crisis contacts if elevated risk"
    referrals: "Therapy, case management, substance treatment, medical workup"
    follow_up: "Next visit timing and contingencies"
post_record_skill: minutes-psych-review
---
```

ASEPTIC is a commonly taught mnemonic for the MSE; it is not APA-canonical. The field set above follows APA Practice Guidelines and StatPearls rather than the mnemonic. Mood and affect are grouped because clinicians frequently document them together; deployments that prefer them split can override.

#### `pediatric-soap` (community, extends `soap`)

Aligned with AAP Bright Futures well-child visit components. Adds growth percentiles, developmental screening, and an immunization-aware Plan section.

```yaml
---
name: Pediatric SOAP Note
slug: pediatric-soap
extends: soap
version: 1.0.0
description: Pediatric SOAP note covering acute visits and Bright Futures-aligned well-child checks.
keywords: [pediatrics, "well-child", "bright futures", soap]
triggers:
  calendar_keywords: [pediatric, peds, "well-child", infant, child, immunization]
  transcript_keywords: [milestones, growth, immunizations, "parent reports"]
extract:
  patient_context:
    age_sex: "Patient age and sex"
    visit_type: "Well-child visit, acute, follow-up"
    historian: "Caregiver and reliability; child if age-appropriate"
  subjective:
    chief_complaint: "Reason for visit (acute symptom or scheduled well-child check)"
    history_present_illness: "HPI; for infants and young children, primarily caregiver-reported"
    birth_history: "Gestational age, delivery, birth weight, neonatal complications"
    feeding_history: "Breast, formula, solids; intake patterns and weight gain"
    developmental_history: "Milestone status: gross motor, fine motor, language, social"
    past_medical_history: "Chronic conditions, prior hospitalizations, ED visits"
    immunization_status: "Up to date per CDC/AAP schedule, or specific gaps"
    family_history: "Pertinent inherited conditions, maternal pregnancy complications"
    social_history: "Caregivers in home, daycare, school grade, screen time, food security"
    medications: "Current medications and recent antibiotics"
    allergies: "Drug, food, environmental allergies with reaction and severity"
  objective:
    vitals: "Weight, length/height, head circumference (birth through 24 months), BMI-for-age (from 24 months), temperature, HR, RR, SpO2; BP risk-based before age 3 and routine from age 3 onward"
    growth_percentiles: "Weight-for-age, length-for-age, weight-for-length (birth through 24 months) or BMI-for-age (from 24 months) on WHO (birth to 2 years) or CDC (2 years and older) curves"
    developmental_screen: "Standardized screen result (ASQ, M-CHAT-R, PEDS, Survey of Wellbeing of Young Children) when administered"
    physical_exam:
      general: "Appearance, hydration, alertness, interaction with caregiver"
      heent: "Fontanelles (infant), tympanic membranes, oropharynx, dentition"
      cardiovascular: "Rate, rhythm, murmurs, femoral pulses (infant)"
      # Additional systems (respiratory, abdominal, GU, neuro, MSK, skin) in the bundled template; abbreviated here.
    labs: "Lead screen, hemoglobin, newborn screen status, other indicated labs"
  assessment:
    primary_diagnosis: "Most likely diagnosis or well-child status"
    differential_diagnoses: "Ranked differentials when symptomatic"
    well_child_findings: "Growth, development, nutrition, behavior status"
    risk_factors: "Identified social, developmental, or medical concerns"
  plan:
    diagnostic: "Tests ordered or recommended"
    therapeutic: "Medications, supportive care"
    immunizations_administered: "Per NCVIA/CDC documentation: vaccine name, date administered, manufacturer, lot number, VIS edition date and date provided to parent/legal representative/patient as applicable, administrator name and address and title; route, dose, and site as best practice"
    anticipatory_guidance: "Visit-specific Bright Futures priorities for this age, including the family's stated agenda and counseling topics actually covered"
    referrals: "Subspecialty, early intervention, developmental services"
    follow_up: "Next well-child or follow-up timing"
post_record_skill: minutes-pediatric-review
---
```

## Implementation phases

### Phase 1: shippable, resolves #143
- `crates/core/src/template.rs`: Template struct, loader, resolver (project > user > bundled)
- `additional_instructions` appended to `build_system_prompt` (base prompt preserved)
- `--template <slug>` flag in CLI
- Bundled templates: `meeting`, `standup`, `1-on-1`, `voice-memo`
- CLI: `minutes template list`, `show`, `validate`
- Tests: loader, resolver, prompt composition, schema validation

### Phase 2: custom extract fields
- Summarizer reads `extract:` from active template, requests structured output from LLM
- Nested objects supported (max 3 levels)
- YAML frontmatter writer extends output with custom fields
- Reader crate (`crates/reader/`, `crates/sdk/src/reader.ts`) passes custom fields through
- MCP tools expose custom fields in responses
- Ship: `interview`, `sales-discovery`, `lecture`

### Phase 3: compliance + post_record_skill
- Compliance rules enforced pre-persistence (redaction, validation)
- `post_record_skill` wired into post-record hook
- `agent_context` injected into MCP responses
- Ship standalone compliance-aware example templates: `soap`, `medical-fr-base`, `therapy-intake`, `legal-consult`
- The US (`soap`) and FR (`medical-fr-base`) families ship independently within Phase 3; a stalled review on one does not block the other.
- Audit log infrastructure

### Phase 4: calendar routing, community gallery, inheritance
- Calendar keyword auto-selection
- `templates/` dir in repo accepting community PRs
- Template validation in CI (schema + smoke test against `example.md`)
- `minutes template install` for gh repos + gists
- `extends:` inheritance resolution
- Inherited specialty children: `psych-soap`, `pediatric-soap` (US family); `consultation-fr`, `soap-fr` (FR family)
- Gallery landing page on useminutes.app

### Phase 5: graph schema extensions
- Custom extract fields flow into `graph.db`
- Domain-aware MCP graph tools (query across medical, legal, sales templates)
- Cross-template queries ("all SOAP notes where diagnosis includes X")

## Open questions

These are the areas where community input most changes the shape. Comments welcome on any of them.

1. **Multi-template composition**: Some recordings straddle domains (a lunch conversation that's half personal + half clinical). Should Minutes support `--templates soap,voice-memo` that merges? My lean is no, too much complexity for marginal benefit, but open to a strong use case.

2. **Template signing**: Community templates declare `agent_context` that gets injected into LLM prompts, which is adjacent to prompt-injection territory. Should templates be signed, reviewed first-party, or sandboxed? My lean: first-party review via PR is enough for Phase 4, signing is a Phase 6+ concern.

3. **Template versioning and re-processing**: If a template bumps major version, do previously-processed meetings get re-processed? My lean: immutable by default; `minutes process <file> --template soap@2.0` is an explicit re-processing action. `version` in frontmatter records which template version was used.

4. **Engine compatibility**: Some templates may rely on structured-output features of specific LLMs (JSON mode in OpenAI/Claude). Should templates declare `supported_engines:` and error loudly on incompatible engines? My lean: yes, fail fast is better than silent degradation, especially for compliance-sensitive templates where partial extraction could be worse than none.

5. **Live transcript interaction**: Live mode streams utterances incrementally. Does the `extract:` schema progress during a live meeting, or only finalize at stop? My lean: finalize at stop for Phase 2 simplicity; streaming structured extraction is a later design.

6. **Compliance extensibility**: Should `compliance` be pluggable so that a Rust plugin could define new rules (e.g., `hipaa_bulk_export_check`)? My lean: Phase 5 or later; start with the fixed enum in `forbid_in_summary` and expand as needs surface.

7. **Inheritance depth**: Single-parent with transitive grandparents, or should we support multiple inheritance (mixins)? My lean: single-parent, explicit and auditable.

8. **Template search / discovery**: CLI `minutes template search` queries bundled + installed; should it also query the gh repo's `templates/` directory? My lean: yes, via a periodic index refresh, not live queries per search.

9. **HIPAA scope of the `soap` template family**: Should the bundled `soap` template explicitly disclaim certification scope in its docstring section, or is the distinction implicit enough that disclaiming risks sounding apologetic? `audit_log: true` and `require_local_processing: true` are tool-level safety behaviors, not deployment-level certifications. HIPAA compliance is ultimately a property of deployment (BAAs, infrastructure, access controls, workforce training), not file format. The Security Rule's six-year retention requirement (45 CFR 164.316(b)(2)(i)) applies to required Security Rule documentation, not raw audit logs or medical records, and medical-record retention is governed by state law and other applicable retention obligations. The audit-controls citation (45 CFR 164.312(b)) and the activity-review citation (45 CFR 164.308(a)(1)(ii)(D)) support a richer audit_log design, and 45 CFR 164.312(a)(2)(i) covers unique user identification, but those references support a richer design rather than spelling out specific required fields. My lean: explicit, terse disclaimer in the bundled template's human-readable section. A follow-up open question (possibly Q10 in a later revision) on whether `audit_log` should expand to capture actor identity and per-record access for deployments that want a stronger trail.

## Acknowledgments

- **@ed0c** (#143) surfaced the limitation and agreed to co-author the regulated-vertical reference implementation. The HDS + RGPD constraints shaped the compliance field directly.
- The `soap` US clinical family draws on published clinical references: StatPearls/NCBI for SOAP structure, AAP Bright Futures for pediatrics, and APA Practice Guidelines for the Mental Status Exam. Clinical reviewers welcome; comment on PR #147.

## Next steps

- Collect feedback on this RFC from clinical reviewers (US and FR families), particularly on inheritance and compliance shapes
- @ed0c and @silverstein converge on `medical-fr-base`, `consultation-fr`, `soap-fr`
- Begin Phase 1 implementation on a separate branch
- RFC merges once feedback has converged
