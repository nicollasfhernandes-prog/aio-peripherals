import { useEffect, useRef, useState } from "react";
import { tr } from "../i18n";
import { setSensor, type SensorSettings } from "../api";

const TOGGLES: { key: "angleSnap" | "rippleControl" | "motionSync"; label: string; hint: string }[] = [
  { key: "angleSnap", label: "Angle snapping", hint: "Endireita movimentos quase horizontais ou verticais." },
  { key: "rippleControl", label: "Ripple control", hint: "Suaviza a tremedeira do sensor em DPI alto (adiciona um pouco de latência)." },
  { key: "motionSync", label: "Motion sync", hint: "Sincroniza a leitura do sensor com o polling USB para um rastreio mais estável." },
];

export function SensorPanel({ id, sensor, onError }: { id: string; sensor: SensorSettings; onError: (m: string) => void }) {
  const [s, setS] = useState(sensor);
  const timer = useRef<number>(undefined);
  useEffect(() => setS(sensor), [JSON.stringify(sensor)]);

  const apply = (key: string, value: number, patch: Partial<SensorSettings>, debounce = 0) => {
    setS((prev) => ({ ...prev, ...patch }));
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setSensor(id, key, value).catch((e) => onError(String(e))), debounce);
  };

  return (
    <section className="glass panel">
      <h3>{tr("Sensor")}</h3>
      <div className="toggle-list">
        {TOGGLES.filter((t) => s[t.key] !== null).map((t) => (
          <div key={t.key} className="toggle-row">
            <div>
              <div className="toggle-label">{tr(t.label)}</div>
              <div className="hint">{tr(t.hint)}</div>
            </div>
            <label className="switch">
              <input
                type="checkbox"
                checked={!!s[t.key]}
                onChange={(e) => apply(t.key, e.target.checked ? 1 : 0, { [t.key]: e.target.checked })}
              />
              <span />
            </label>
          </div>
        ))}
      </div>

      {s.lod !== null && s.lodOptions.length > 0 && (
        <>
          <label className="slider-label">
            <span>{tr("Altura de levantamento (LOD)")}</span>
          </label>
          <div className="segmented">
            {s.lodOptions.map(([value, label]) => (
              <button key={value} className={s.lod === value ? "active" : ""} onClick={() => apply("lod", value, { lod: value })}>
                {label}
              </button>
            ))}
          </div>
        </>
      )}

      {s.debounceMs !== null && (
        <>
          <label className="slider-label">
            <span>{tr("Debounce do clique")}</span>
            <span>{s.debounceMs} ms</span>
          </label>
          <input
            type="range"
            min={0}
            max={s.debounceMax}
            value={s.debounceMs}
            onChange={(e) => {
              const v = Number(e.target.value);
              apply("debounceMs", v, { debounceMs: v }, 200);
            }}
          />
          <p className="hint">{tr("Menor é mais rápido, mas baixo demais pode gerar cliques duplos.")}</p>
        </>
      )}
    </section>
  );
}
