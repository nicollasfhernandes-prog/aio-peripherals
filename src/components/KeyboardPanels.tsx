import { useEffect, useRef, useState } from "react";
import { setTrigger, startKeyMonitor, type Actuation, type Trigger } from "../api";
import { tr } from "../i18n";
import { CUSTOM_MODE, toHex, type KeyboardLightingState } from "./Lighting";
import { WIN60 } from "../layouts/win60";

const MODES = [
  { id: 0, label: "Fixo" },
  { id: 12, label: "Rapid Trigger" },
  { id: 13, label: "RT separado" },
];

const sameTrigger = (a: Trigger, b: Trigger) =>
  a.mode === b.mode && a.travel === b.travel && a.press === b.press && a.release === b.release;

const pickTrigger = ({ mode, travel, press, release }: Trigger): Trigger => ({ mode, travel, press, release });

export type KeyEditor = ReturnType<typeof useKeyEditor>;

/** Per-key trigger state with a selection; edits go to the selected keys (or all keys when none are selected). */
export function useKeyEditor(id: string, actuation: Actuation | null, onError: (m: string) => void) {
  const [keys, setKeys] = useState(() => new Map(actuation?.keys.map((k) => [k.index, pickTrigger(k)])));
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const timer = useRef<number>(undefined);

  // Resync from the device after a rescan.
  useEffect(() => {
    setKeys(new Map(actuation?.keys.map((k) => [k.index, pickTrigger(k)])));
  }, [JSON.stringify(actuation?.keys)]);

  const targets = selected.size > 0 ? [...selected] : [...keys.keys()];
  const targetTriggers = targets.map((i) => keys.get(i)).filter((t): t is Trigger => !!t);
  const current = targetTriggers[0] ?? null;
  const mixed = targetTriggers.some((t) => current && !sameTrigger(t, current));

  const apply = (t: Trigger) => {
    const next = new Map(keys);
    targets.forEach((i) => next.set(i, t));
    setKeys(next);
    const list = selected.size > 0 ? targets : [];
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setTrigger(id, t, list).catch((e) => onError(String(e))), 150);
  };

  const toggle = (index: number, on?: boolean) =>
    setSelected((prev) => {
      const next = new Set(prev);
      (on ?? !next.has(index)) ? next.add(index) : next.delete(index);
      return next;
    });

  return { keys, selected, setSelected, toggle, current, mixed, apply, targets };
}

export interface LiveTravel {
  travel: Map<number, number>;
  peak: Map<number, number>;
  last: number | null;
  reset: () => void;
}

/** Live key travel from the keyboard's test mode while `enabled`. */
export function useLiveTravel(id: string, enabled: boolean, onError: (m: string) => void): LiveTravel | null {
  const [travel, setTravel] = useState(new Map<number, number>());
  const [peak, setPeak] = useState(new Map<number, number>());
  const [last, setLast] = useState<number | null>(null);

  useEffect(() => {
    if (!enabled) return;
    let stop: (() => void) | null = null;
    let cancelled = false;
    startKeyMonitor(id, (batch) => {
      setTravel((prev) => {
        const next = new Map(prev);
        batch.forEach(([k, v]) => next.set(k, v));
        return next;
      });
      setPeak((prev) => {
        const next = new Map(prev);
        batch.forEach(([k, v]) => v > (next.get(k) ?? 0) && next.set(k, v));
        return next;
      });
      const moving = batch.filter(([, v]) => v > 0);
      if (moving.length) setLast(moving.reduce((a, b) => (b[1] > a[1] ? b : a))[0]);
    })
      .then((s) => (cancelled ? s() : (stop = s)))
      .catch((e) => onError(String(e)));
    return () => {
      cancelled = true;
      stop?.();
      setTravel(new Map());
    };
  }, [id, enabled]);

  const reset = () => {
    setPeak(new Map());
    setLast(null);
  };
  return enabled ? { travel, peak, last, reset } : null;
}

const MODE_CLASS: Record<number, string> = { 12: "rt", 13: "rt-split" };

export type KeyboardView = "actuation" | "lighting" | "live";

interface KeyboardProps {
  editor: KeyEditor;
  actuation: Actuation;
  live: LiveTravel | null;
  lighting: KeyboardLightingState;
  view: KeyboardView;
  onViewChange: (v: KeyboardView) => void;
}

const VIEWS: { id: KeyboardView; label: string }[] = [
  { id: "actuation", label: "Atuação" },
  { id: "lighting", label: "Iluminação" },
  { id: "live", label: "Teste ao vivo" },
];

export function InteractiveKeyboard({ editor, actuation, live, lighting, view, onViewChange }: KeyboardProps) {
  const painting = view === "lighting";
  const light = lighting.light;
  const unitMm = actuation.unitMm;
  const dragging = useRef<boolean | null>(null); // value being painted while dragging
  const width = Math.max(...WIN60.map((k) => k.x + k.w)) + 10;
  const height = Math.max(...WIN60.map((k) => k.y + k.h)) + 15;

  useEffect(() => {
    const stop = () => (dragging.current = null);
    window.addEventListener("pointerup", stop);
    return () => window.removeEventListener("pointerup", stop);
  }, []);

  const select = (indices: number[]) => editor.setSelected(new Set(indices));
  const byLabel = (labels: string[]) => WIN60.filter((k) => labels.includes(k.label)).map((k) => k.index);

  return (
    <div className="kb-editor">
      <div className="segmented kb-tabs">
        {VIEWS.map((v) => (
          <button key={v.id} className={view === v.id ? "active" : ""} onClick={() => onViewChange(v.id)}>
            {tr(v.label)}
          </button>
        ))}
      </div>

      {painting ? (
        <div className="kb-toolbar">
          <span className="muted">
            {light?.mode === CUSTOM_MODE
              ? tr("Clique ou arraste sobre as teclas para pintar com o pincel.")
              : tr("Escolha o efeito Personalizado (ou comece a pintar) para colorir tecla por tecla.")}
          </span>
          <span className="brush-preview" style={{ background: toHex(lighting.brush) }} title="Brush" />
        </div>
      ) : live ? (
        <div className="kb-toolbar">
          <span className="muted">{tr("Aperte qualquer tecla para ver o quanto ela desce.")}</span>
          <div className="chips">
            <button className="chip" onClick={live.reset}>
              {tr("Zerar picos")}
            </button>
          </div>
        </div>
      ) : (
      <div className="kb-toolbar">
        <span className="muted">
          {editor.selected.size === 0
            ? tr("Clique ou arraste sobre as teclas para selecionar. Sem seleção = todas as teclas.")
            : tr(editor.selected.size > 1 ? "{n} teclas selecionadas" : "{n} tecla selecionada", { n: editor.selected.size })}
        </span>
        <div className="chips">
          <button className="chip" onClick={() => select(byLabel(["W", "A", "S", "D"]))}>
            WASD
          </button>
          <button className="chip" onClick={() => select(WIN60.map((k) => k.index))}>
            {tr("Todas")}
          </button>
          <button className="chip" onClick={() => select([])} disabled={editor.selected.size === 0}>
            {tr("Limpar")}
          </button>
        </div>
      </div>
      )}

      <svg viewBox={`0 0 ${width} ${height}`} className="kb-svg" onContextMenu={(e) => e.preventDefault()}>
        <rect x="0" y="5" width={width} height={height - 5} rx="12" className="kb-case" />
        {WIN60.map((k) => {
          const t = editor.keys.get(k.index);
          const sel = !live && !painting && editor.selected.has(k.index);
          const keyColor = painting ? lighting.colors.get(k.index) : undefined;
          const pos = live?.travel.get(k.index) ?? 0;
          const fill = Math.min(1, pos / actuation.max);
          const capH = k.h - 8;
          const actuated = live && t && pos > 0 && pos >= t.travel;
          const cls = [
            "kb-key",
            t ? MODE_CLASS[t.mode] ?? "" : "",
            sel ? "selected" : "",
            live ? "live" : "",
            painting ? "paint" : "",
            actuated ? "actuated" : "",
          ].join(" ");
          const value = painting ? null : live ? (pos > 0 ? pos : null) : t?.travel ?? null;
          return (
            <g
              key={k.index}
              className={cls}
              onPointerDown={(e) => {
                if (live) return;
                e.preventDefault();
                if (painting) {
                  dragging.current = true;
                  lighting.paint([k.index]);
                  return;
                }
                dragging.current = !sel;
                editor.toggle(k.index, !sel);
              }}
              onPointerEnter={() => {
                if (live || dragging.current === null) return;
                if (painting) lighting.paint([k.index]);
                else editor.toggle(k.index, dragging.current);
              }}
            >
              <rect x={k.x + 1} y={k.y + 1} width={k.w - 2} height={k.h - 2} rx="6" className="kb-cap-base" />
              <rect x={k.x + 3} y={k.y + 2} width={k.w - 6} height={capH} rx="5" className="kb-cap" />
              {keyColor && toHex(keyColor) !== "#000000" && (
                <rect
                  x={k.x + 3}
                  y={k.y + 2}
                  width={k.w - 6}
                  height={capH}
                  rx="5"
                  className="kb-led"
                  style={{ fill: toHex(keyColor) }}
                />
              )}
              {fill > 0 && (
                <rect
                  x={k.x + 3}
                  y={k.y + 2 + capH * (1 - fill)}
                  width={k.w - 6}
                  height={capH * fill}
                  rx="5"
                  className="kb-fill"
                />
              )}
              <text x={k.x + 7} y={k.y + 13} className="kb-label">
                {k.label}
              </text>
              {value !== null && (
                <text x={k.x + k.w / 2} y={k.y + k.h - 11} className="kb-value" textAnchor="middle">
                  {(value * unitMm).toFixed(2)}
                </text>
              )}
            </g>
          );
        })}
      </svg>

      {painting ? (
        <div className="kb-legend">
          <span className="muted">{tr("Cores = layout do efeito Personalizado (preto = apagada)")}</span>
        </div>
      ) : live ? (
        <div className="kb-legend">
          <span>
            <i className="dot travel" /> {tr("Curso")}
          </span>
          <span>
            <i className="dot actuated" /> {tr("Passou do ponto de atuação")}
          </span>
          <span className="muted">{tr("Valores = profundidade atual (mm)")}</span>
        </div>
      ) : (
      <div className="kb-legend">
        <span>
          <i className="dot fixed" /> {tr("Fixo")}
        </span>
        <span>
          <i className="dot rt" /> Rapid Trigger
        </span>
        <span>
          <i className="dot rt-split" /> {tr("RT separado")}
        </span>
        <span className="muted">{tr("Valores = ponto de atuação (mm)")}</span>
      </div>
      )}
    </div>
  );
}

/** Big readout for the most recently pressed key, like a single-key travel meter. */
export function LivePanel({ live, editor, actuation }: { live: LiveTravel; editor: KeyEditor; actuation: Actuation }) {
  const key = live.last !== null ? WIN60.find((k) => k.index === live.last) : undefined;
  const pos = key ? live.travel.get(key.index) ?? 0 : 0;
  const peak = key ? live.peak.get(key.index) ?? 0 : 0;
  const trig = key ? editor.keys.get(key.index) : undefined;
  const pct = (v: number) => `${Math.min(100, (v / actuation.max) * 100)}%`;
  const mm = (v: number) => (v * actuation.unitMm).toFixed(2);

  return (
    <section className="glass panel">
      <div className="panel-head">
        <h3>{tr("Curso ao vivo")}</h3>
        <span className="big">
          {mm(pos)} <small>mm</small>
        </span>
      </div>
      <div className="live-meter">
        <div className="meter">
          <div className="meter-fill" style={{ height: pct(pos) }} />
          <div className="meter-peak" style={{ top: pct(peak) }} />
          {trig && <div className="meter-actuation" style={{ top: pct(trig.travel) }} />}
        </div>
        <dl>
          <dt>{tr("Tecla")}</dt>
          <dd>{key ? key.label || tr("Espaço") : "-"}</dd>
          <dt>{tr("Pico")}</dt>
          <dd>{key ? `${mm(peak)} mm` : "-"}</dd>
          <dt>{tr("Atuação")}</dt>
          <dd>{trig ? `${mm(trig.travel)} mm` : "-"}</dd>
          <dt>{tr("Modo")}</dt>
          <dd>{trig ? tr(MODES.find((m) => m.id === trig.mode)?.label ?? String(trig.mode)) : "-"}</dd>
        </dl>
      </div>
      <p className="hint">
        {tr("Linha verde = ponto de atuação, linha branca = pressão mais funda. Curso total {mm} mm.", { mm: mm(actuation.max) })}
      </p>
    </section>
  );
}

export function ActuationPanel({ editor, actuation }: { editor: KeyEditor; actuation: Actuation }) {
  const t = editor.current;
  if (!t) return null;
  const mm = (units: number) => (units * actuation.unitMm).toFixed(2);
  const rtMax = Math.round(actuation.max / 2);
  const scope =
    editor.selected.size === 0 ? tr("Todas as teclas") : tr(editor.selected.size > 1 ? "{n} teclas selecionadas" : "{n} tecla selecionada", { n: editor.selected.size });

  return (
    <section className="glass panel">
      <div className="panel-head">
        <h3>{tr("Atuação")}</h3>
        <span className="big">
          {mm(t.travel)} <small>mm</small>
        </span>
      </div>
      <p className="scope">
        {scope}
        {editor.mixed && <span className="pill small">{tr("Valores diferentes: editar iguala todas")}</span>}
      </p>
      <div className="segmented">
        {MODES.map((m) => (
          <button key={m.id} className={t.mode === m.id ? "active" : ""} onClick={() => editor.apply({ ...t, mode: m.id })}>
            {tr(m.label)}
          </button>
        ))}
      </div>
      <label className="slider-label">
        <span>{tr("Ponto de atuação")}</span>
        <span>{mm(t.travel)} mm</span>
      </label>
      <input
        type="range"
        min={actuation.min}
        max={actuation.max}
        value={t.travel}
        onChange={(e) => editor.apply({ ...t, travel: Number(e.target.value) })}
      />
      {t.mode !== 0 && (
        <>
          <label className="slider-label">
            <span>{tr(t.mode === 13 ? "Sensibilidade ao apertar" : "Sensibilidade do Rapid Trigger")}</span>
            <span>{mm(t.press)} mm</span>
          </label>
          <input
            type="range"
            min={1}
            max={rtMax}
            value={t.press}
            onChange={(e) => {
              const v = Number(e.target.value);
              editor.apply({ ...t, press: v, release: t.mode === 13 ? t.release : v });
            }}
          />
        </>
      )}
      {t.mode === 13 && (
        <>
          <label className="slider-label">
            <span>{tr("Sensibilidade ao soltar")}</span>
            <span>{mm(t.release)} mm</span>
          </label>
          <input
            type="range"
            min={1}
            max={rtMax}
            value={t.release}
            onChange={(e) => editor.apply({ ...t, release: Number(e.target.value) })}
          />
        </>
      )}
      <p className="hint">
        {tr("Curso de {min} a {max} mm.", { min: mm(actuation.min), max: mm(actuation.max) })}
      </p>
    </section>
  );
}
