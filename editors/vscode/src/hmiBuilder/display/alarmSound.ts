// Sound on the runtime display (specification 39, §5.9 item 2; EEMUA 191 practice): a High alarm
// sounds until it is acknowledged or silenced, a Medium alarm once when it is raised, a Low alarm
// not at all. Silence stops the sound without acknowledging; an alarm raised later sounds again.
// The display sounds where its top bar shows the alarms, beside which Silence appears.

import type { AlarmRowModel, AlarmSummaryModel } from "../drawing/model";

/** Plays the tones: "high" repeats until stopped, "medium" plays once. */
export interface AlarmSounds {
  play(pattern: "high" | "medium"): void;
  stop(): void;
}

/** An alarm's raise: a new raise has a new activation time. */
const raise = (row: AlarmRowModel): string => `${row.path}@${row.since_ms}`;

export class AlarmSound {
  /** The raises that have sounded once, and the High raises silenced. */
  private readonly heard = new Set<string>();
  private readonly silenced = new Set<string>();
  private loud = false;

  constructor(private readonly sounds: AlarmSounds | undefined) {}

  /** Whether a High alarm sounds now. */
  get sounding(): boolean {
    return this.loud;
  }

  /** The alarms after an update. */
  update(summary: AlarmSummaryModel | undefined): void {
    if (!this.sounds) return;
    const active = summary?.active ?? [];
    for (const row of active) {
      if (row.priority === "medium" && row.unacknowledged && !this.heard.has(raise(row))) {
        this.heard.add(raise(row));
        this.sounds.play("medium");
      }
    }
    const present = new Set(active.map(raise));
    for (const key of [...this.heard, ...this.silenced]) {
      if (!present.has(key)) {
        this.heard.delete(key);
        this.silenced.delete(key);
      }
    }
    const loud = active.some((row) => row.priority === "high" && row.unacknowledged && !this.silenced.has(raise(row)));
    if (loud && !this.loud) this.sounds.play("high");
    else if (!loud && this.loud) this.sounds.stop();
    this.loud = loud;
  }

  /** Silence: the High alarms sounding now stop and stay unacknowledged. */
  silence(summary: AlarmSummaryModel | undefined): void {
    for (const row of summary?.active ?? []) {
      if (row.priority === "high" && row.unacknowledged) this.silenced.add(raise(row));
    }
    if (this.loud) this.sounds?.stop();
    this.loud = false;
  }
}

/** The browser's tones through Web Audio. A browser plays sound only after the page was touched or
 * clicked, so the first press starts the audio. */
export function browserSounds(window: Window): AlarmSounds {
  let context: AudioContext | undefined;
  let repeat: number | undefined;
  const audio = (): AudioContext | undefined => {
    try {
      context ??= new AudioContext();
      if (context.state === "suspended") void context.resume();
      return context;
    } catch {
      return undefined;
    }
  };
  window.addEventListener("pointerdown", () => void audio(), { capture: true });
  const tone = (frequency: number, startIn: number, seconds: number): void => {
    const sound = audio();
    if (!sound) return;
    const oscillator = sound.createOscillator();
    const gain = sound.createGain();
    const start = sound.currentTime + startIn;
    oscillator.frequency.value = frequency;
    gain.gain.setValueAtTime(0.2, start);
    gain.gain.exponentialRampToValueAtTime(0.0001, start + seconds);
    oscillator.connect(gain).connect(sound.destination);
    oscillator.start(start);
    oscillator.stop(start + seconds);
  };
  return {
    play(pattern) {
      if (pattern === "medium") {
        tone(660, 0, 0.25);
        tone(660, 0.35, 0.25);
        return;
      }
      if (repeat !== undefined) return;
      const pulse = (): void => {
        tone(880, 0, 0.2);
        tone(880, 0.3, 0.2);
      };
      pulse();
      repeat = window.setInterval(pulse, 1500);
    },
    stop() {
      if (repeat !== undefined) window.clearInterval(repeat);
      repeat = undefined;
    },
  };
}
