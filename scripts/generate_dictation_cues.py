#!/usr/bin/env python3
from __future__ import annotations

import math
import wave
from pathlib import Path

SAMPLE_RATE = 44_100


def ease_out_quad(t: float) -> float:
    return 1.0 - (1.0 - t) * (1.0 - t)


def envelope(index: int, total: int, attack: float, release: float) -> float:
    attack_samples = max(1, int(total * attack))
    release_samples = max(1, int(total * release))
    if index < attack_samples:
        return index / attack_samples
    if index > total - release_samples:
        return max(0.0, (total - index) / release_samples)
    return 1.0


def render_tone(freq: float, duration: float, gain: float = 0.35) -> list[float]:
    total = int(SAMPLE_RATE * duration)
    data: list[float] = []
    for i in range(total):
        t = i / SAMPLE_RATE
        env = envelope(i, total, 0.12, 0.28)
        wobble = 1.0 + 0.0009 * math.sin(2 * math.pi * 5.5 * t)
        base = math.sin(2 * math.pi * freq * wobble * t)
        harmonic = 0.32 * math.sin(2 * math.pi * freq * 2.01 * t + 0.2)
        shimmer = 0.14 * math.sin(2 * math.pi * freq * 3.98 * t)
        data.append((base + harmonic + shimmer) * env * gain)
    return data


def silence(duration: float) -> list[float]:
    return [0.0] * int(SAMPLE_RATE * duration)


def mix(parts: list[list[float]]) -> list[float]:
    total = max(len(part) for part in parts)
    out = [0.0] * total
    for part in parts:
        for i, sample in enumerate(part):
            out[i] += sample
    return out


def normalize(samples: list[float], peak: float = 0.82) -> list[float]:
    maximum = max(abs(sample) for sample in samples) or 1.0
    scale = peak / maximum
    return [sample * scale for sample in samples]


def chain(segments: list[list[float]]) -> list[float]:
    out: list[float] = []
    for segment in segments:
        out.extend(segment)
    return out


def write_wav(path: Path, samples: list[float]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    pcm = bytearray()
    for sample in normalize(samples):
        clipped = max(-1.0, min(1.0, sample))
        pcm.extend(int(clipped * 32767.0).to_bytes(2, "little", signed=True))
    with wave.open(str(path), "wb") as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(SAMPLE_RATE)
        wav.writeframes(bytes(pcm))


def build_start() -> list[float]:
    return chain(
        [
            render_tone(523.25, 0.055, 0.22),
            silence(0.012),
            render_tone(698.46, 0.085, 0.30),
        ]
    )


def build_stop() -> list[float]:
    return chain(
        [
            render_tone(698.46, 0.05, 0.22),
            silence(0.01),
            render_tone(466.16, 0.095, 0.28),
        ]
    )


def build_complete() -> list[float]:
    return chain(
        [
            render_tone(587.33, 0.04, 0.16),
            silence(0.01),
            render_tone(739.99, 0.045, 0.18),
            silence(0.012),
            render_tone(698.46, 0.095, 0.22),
        ]
    )


def build_error() -> list[float]:
    first = mix([render_tone(392.00, 0.08, 0.20), render_tone(415.30, 0.08, 0.13)])
    second = mix([render_tone(329.63, 0.10, 0.18), render_tone(311.13, 0.10, 0.11)])
    return chain([first, silence(0.02), second])


def main() -> None:
    audio_dir = Path(__file__).resolve().parents[1] / "tauri" / "src" / "audio"
    cues = {
        "cue-start.wav": build_start(),
        "cue-stop.wav": build_stop(),
        "cue-complete.wav": build_complete(),
        "cue-error.wav": build_error(),
    }
    for name, samples in cues.items():
        write_wav(audio_dir / name, samples)
        duration = len(samples) / SAMPLE_RATE
        print(f"wrote {name} ({duration:.3f}s)")


if __name__ == "__main__":
    main()
