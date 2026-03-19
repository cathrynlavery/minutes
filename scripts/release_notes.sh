#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "usage: $0 <from-ref> <to-ref> [channel]" >&2
  exit 1
fi

from_ref="$1"
to_ref="$2"
channel="${3:-stable}"

if ! git rev-parse --verify "${from_ref}^{commit}" >/dev/null 2>&1; then
  echo "could not resolve from-ref: ${from_ref}" >&2
  exit 1
fi

if ! git rev-parse --verify "${to_ref}^{commit}" >/dev/null 2>&1; then
  echo "could not resolve to-ref: ${to_ref}" >&2
  exit 1
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

core_file="$tmpdir/core.txt"
cli_file="$tmpdir/cli.txt"
desktop_file="$tmpdir/desktop.txt"
mcp_file="$tmpdir/mcp.txt"
other_file="$tmpdir/other.txt"

touch "$core_file" "$cli_file" "$desktop_file" "$mcp_file" "$other_file"

while IFS=$'\t' read -r sha subject; do
  [[ -z "$sha" ]] && continue

  paths="$(git diff-tree --no-commit-id --name-only -r "$sha")"
  line="- ${subject} (\`${sha:0:7}\`)"
  matched=0

  if grep -q '^crates/cli/' <<<"$paths"; then
    echo "$line" >> "$cli_file"
    matched=1
  fi
  if grep -q '^tauri/' <<<"$paths"; then
    echo "$line" >> "$desktop_file"
    matched=1
  fi
  if grep -q '^crates/mcp/' <<<"$paths"; then
    echo "$line" >> "$mcp_file"
    matched=1
  fi
  if grep -q '^crates/core/' <<<"$paths"; then
    echo "$line" >> "$core_file"
    matched=1
  fi

  if [[ "$matched" -eq 0 ]]; then
    echo "$line" >> "$other_file"
  fi
done < <(git log --reverse --pretty=format:'%H%x09%s' "${from_ref}..${to_ref}")

count_lines() {
  local file="$1"
  if [[ -s "$file" ]]; then
    wc -l < "$file" | tr -d ' '
  else
    echo 0
  fi
}

cli_count="$(count_lines "$cli_file")"
desktop_count="$(count_lines "$desktop_file")"
mcp_count="$(count_lines "$mcp_file")"
core_count="$(count_lines "$core_file")"
other_count="$(count_lines "$other_file")"

if [[ "$channel" == "preview" ]]; then
  channel_sentence="Preview release intended for early adopters and maintainers."
else
  channel_sentence="Stable release intended for broad usage."
fi

echo "# Release Notes for ${to_ref}"
echo
echo "- Channel: ${channel}"
echo "- Range: ${from_ref}..${to_ref}"
echo "- ${channel_sentence}"
echo
echo "## What changed"
echo
if [[ "$core_count" -gt 0 ]]; then
  echo "### Shared engine"
  cat "$core_file"
  echo
fi
if [[ "$desktop_count" -gt 0 ]]; then
  echo "### Desktop"
  cat "$desktop_file"
  echo
fi
if [[ "$cli_count" -gt 0 ]]; then
  echo "### CLI"
  cat "$cli_file"
  echo
fi
if [[ "$mcp_count" -gt 0 ]]; then
  echo "### MCP / agent integrations"
  cat "$mcp_file"
  echo
fi
if [[ "$other_count" -gt 0 ]]; then
  echo "### Other repo changes"
  cat "$other_file"
  echo
fi
if [[ "$core_count" -eq 0 && "$desktop_count" -eq 0 && "$cli_count" -eq 0 && "$mcp_count" -eq 0 && "$other_count" -eq 0 ]]; then
  echo "- No commits found in this range."
  echo
fi

echo "## Who should care"
echo
if [[ "$desktop_count" -gt 0 ]]; then
  echo "- Desktop users should care about this release."
fi
if [[ "$cli_count" -gt 0 ]]; then
  echo "- CLI users should care about this release."
fi
if [[ "$mcp_count" -gt 0 ]]; then
  echo "- MCP / Claude / Codex users should care about this release."
fi
if [[ "$core_count" -gt 0 && "$desktop_count" -eq 0 && "$cli_count" -eq 0 && "$mcp_count" -eq 0 ]]; then
  echo "- Users across multiple surfaces may care because the shared engine changed."
fi
if [[ "$core_count" -eq 0 && "$desktop_count" -eq 0 && "$cli_count" -eq 0 && "$mcp_count" -eq 0 ]]; then
  echo "- This range does not contain user-facing product changes."
fi
echo

echo "## CLI / MCP / desktop impact"
echo
if [[ "$cli_count" -gt 0 ]]; then
  echo "- CLI: user-facing CLI changes are included in this release."
else
  echo "- CLI: no direct CLI changes in this release."
fi
if [[ "$desktop_count" -gt 0 ]]; then
  echo "- Desktop: desktop app changes are included in this release."
else
  echo "- Desktop: no direct desktop changes in this release."
fi
if [[ "$mcp_count" -gt 0 ]]; then
  echo "- MCP: MCP or agent-integration changes are included in this release."
else
  echo "- MCP: no direct MCP or agent-integration changes in this release."
fi
if [[ "$core_count" -gt 0 ]]; then
  echo "- Shared engine: core pipeline changes may affect more than one surface."
fi
echo

echo "## Breaking changes or migration notes"
echo
echo "- None identified automatically. Verify manually before publishing."
echo
echo "## Known issues"
echo
if [[ "$channel" == "preview" ]]; then
  echo "- Preview build: validate install, capture, and recovery flows before recommending broad use."
else
  echo "- No known issues recorded in the generated notes. Add any release-specific caveats before publishing."
fi
