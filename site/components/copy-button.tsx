"use client";

import { useState } from "react";

export function CopyButton({ label, cmd }: { label: string; cmd: string }) {
  const [copied, setCopied] = useState(false);

  return (
    <button
      onClick={() => {
        navigator.clipboard.writeText(cmd).then(() => {
          setCopied(true);
          setTimeout(() => setCopied(false), 1500);
        });
      }}
      className="group relative min-w-[220px] cursor-pointer rounded-[8px] border border-[color:var(--border)] bg-[var(--bg-elevated)] px-5 py-3 text-left font-mono text-[13px] text-[var(--text)] shadow-[var(--shadow-panel)] transition-all hover:border-[color:var(--border-mid)] hover:bg-[var(--bg-hover)]"
    >
      <span className="mb-1 block font-sans text-[11px] uppercase tracking-[0.16em] text-[var(--text-secondary)]">
        {label}
      </span>
      {cmd}
      {copied && (
        <span className="absolute inset-0 flex items-center justify-center rounded-[8px] bg-[var(--bg-elevated)] font-sans text-xs text-[var(--green)]">
          Copied!
        </span>
      )}
    </button>
  );
}
