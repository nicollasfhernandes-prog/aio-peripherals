import { useEffect, useState } from "react";
import { startAnalogMonitor } from "../api";
import { tr } from "../i18n";

/** Names for HID keyboard usage codes (what the Wooting analog protocol reports). */
function keyName(code: number): string {
  const c = code & 0xff;
  if (code >> 8) return `0x${code.toString(16)}`;
  if (c >= 0x04 && c <= 0x1d) return String.fromCharCode(65 + c - 0x04);
  if (c >= 0x1e && c <= 0x26) return String(c - 0x1d);
  if (c >= 0x3a && c <= 0x45) return `F${c - 0x39}`;
  const named: Record<number, string> = {
    0x27: "0", 0x28: "Enter", 0x29: "Esc", 0x2a: "⌫", 0x2b: "Tab", 0x2c: tr("Espaço"), 0x2d: "-", 0x2e: "=",
    0x2f: "[", 0x30: "]", 0x31: "\\", 0x33: ";", 0x34: "'", 0x35: "`", 0x36: ",", 0x37: ".", 0x38: "/",
    0x39: "Caps", 0x4f: "→", 0x50: "←", 0x51: "↓", 0x52: "↑", 0xe0: "Ctrl", 0xe1: "Shift", 0xe2: "Alt",
    0xe3: "Win", 0xe4: "Ctrl R", 0xe5: "Shift R", 0xe6: "Alt R", 0xe7: "Win R",
  };
  return named[c] ?? `0x${c.toString(16)}`;
}

/** Live analog depth of every pressed key (Wooting). */
export function AnalogPanel({ id, onError }: { id: string; onError: (m: string) => void }) {
  const [on, setOn] = useState(false);
  const [keys, setKeys] = useState<[number, number][]>([]);
  const [peak, setPeak] = useState(0);

  useEffect(() => {
    if (!on) return;
    let stop: (() => void) | null = null;
    let cancelled = false;
    startAnalogMonitor(id, (frame) => {
      setKeys(frame);
      const max = frame.reduce((m, [, v]) => Math.max(m, v), 0);
      setPeak((p) => Math.max(p, max));
    })
      .then((s) => (cancelled ? s() : (stop = s)))
      .catch((e) => {
        onError(String(e));
        setOn(false);
      });
    return () => {
      cancelled = true;
      stop?.();
      setKeys([]);
    };
  }, [id, on]);

  const sorted = [...keys].sort((a, b) => b[1] - a[1]).slice(0, 8);

  return (
    <section className="glass panel">
      <div className="panel-head">
        <h3>{tr("Analógico ao vivo")}</h3>
        <label className="switch">
          <input type="checkbox" checked={on} onChange={(e) => setOn(e.target.checked)} />
          <span />
        </label>
      </div>
      {on ? (
        <>
          {sorted.length === 0 ? (
            <p className="hint">{tr("Aperte qualquer tecla para ver o quanto ela desce.")}</p>
          ) : (
            <div className="analog-list">
              {sorted.map(([code, value]) => (
                <div key={code} className="analog-row">
                  <span className="analog-key">{keyName(code)}</span>
                  <div className="analog-bar">
                    <div style={{ width: `${value / 10}%` }} />
                  </div>
                  <span className="analog-value">{Math.round(value / 10)}%</span>
                </div>
              ))}
            </div>
          )}
          <p className="hint">
            {tr("Pico: {n}% do curso.", { n: Math.round(peak / 10) })}
          </p>
        </>
      ) : (
        <p className="hint">{tr("Ative para ver a profundidade de cada tecla em tempo real.")}</p>
      )}
    </section>
  );
}
