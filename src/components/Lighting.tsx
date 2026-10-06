import { useEffect, useRef, useState } from "react";
import { setCustomColors, setLed, setLighting, type LedEffect, type LedZone, type Lighting } from "../api";
import { tr, trName } from "../i18n";
import { WIN60 } from "../layouts/win60";

export type Rgb = [number, number, number];

export const toHex = ([r, g, b]: number[]) => "#" + [r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("");
export const fromHex = (h: string): Rgb => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16)) as Rgb;

const SWATCHES: Rgb[] = [
  [255, 255, 255], [255, 0, 0], [255, 96, 0], [255, 200, 0], [0, 255, 64],
  [0, 255, 255], [0, 96, 255], [128, 0, 255], [255, 0, 160], [0, 0, 0],
];

/** Calls `push` with the latest value once edits pause. */
function useDebounced<T>(push: (v: T) => Promise<void>, onError: (m: string) => void, ms = 150) {
  const timer = useRef<number>(undefined);
  return (v: T) => {
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => push(v).catch((e) => onError(String(e))), ms);
  };
}

// ---------------------------------------------------------------- keyboard

export const CUSTOM_MODE = 10;

/** Which extra controls each AULA effect uses (mirrors the official web driver). */
const EFFECT_CONTROLS: Record<number, { speed: boolean; color: boolean; dirs: number[] }> = {
  0: { speed: false, color: true, dirs: [] },
  10: { speed: false, color: false, dirs: [] },
  2: { speed: true, color: true, dirs: [1, 0, 2, 3, 5, 4] },
  4: { speed: true, color: true, dirs: [1, 0] },
  15: { speed: true, color: true, dirs: [1, 0] },
  7: { speed: true, color: true, dirs: [5, 4] },
  12: { speed: false, color: true, dirs: [2, 3] },
};
const DEFAULT_CONTROLS = { speed: true, color: true, dirs: [] as number[] };

const DIRECTION_LABEL: Record<number, string> = { 0: "→ Direita", 1: "← Esquerda", 2: "↑ Cima", 3: "↓ Baixo", 4: "Para fora", 5: "Para dentro" };
const ROTATION_LABEL: Record<number, string> = { 0: "↻ Horário", 1: "↺ Anti-horário" };

export type KeyboardLightingState = ReturnType<typeof useKeyboardLighting>;

/** Keyboard lighting state shared by the panel and the keyboard drawing (for painting custom colours). */
export function useKeyboardLighting(id: string, initial: Lighting | null, onError: (m: string) => void) {
  const [light, setLight] = useState(initial);
  const [colors, setColors] = useState(() => new Map(initial?.custom.map((c) => [c.index, c.color as Rgb])));
  const [brush, setBrush] = useState<Rgb>([255, 0, 0]);
  const pushLight = useDebounced<Lighting>((v) => setLighting(id, v), onError);
  const pushColors = useDebounced<Map<number, Rgb>>(
    (m) => setCustomColors(id, [...m].map(([index, color]) => ({ index, color }))),
    onError,
    250,
  );

  useEffect(() => {
    setLight(initial);
    setColors(new Map(initial?.custom.map((c) => [c.index, c.color as Rgb])));
  }, [JSON.stringify(initial)]);

  const update = (next: Lighting) => {
    setLight(next);
    pushLight(next);
  };

  // Drags fire several paints before React re-renders, so build on the latest map, not the render's.
  const latest = useRef(colors);
  latest.current = colors;
  const paint = (indices: number[], color: Rgb = brush) => {
    const next = new Map(latest.current);
    indices.forEach((i) => next.set(i, color));
    latest.current = next;
    setColors(next);
    pushColors(next);
    // Painting only shows in the Custom effect, so switch to it.
    if (light && light.mode !== CUSTOM_MODE) update({ ...light, mode: CUSTOM_MODE, on: true });
  };

  return { light, update, colors, brush, setBrush, paint };
}

export function KeyboardLightingPanel({ state }: { state: KeyboardLightingState }) {
  const { light, update, brush, setBrush, paint } = state;
  if (!light) return null;
  const ctl = EFFECT_CONTROLS[light.mode] ?? DEFAULT_CONTROLS;
  const rotational = light.mode === 4 || light.mode === 15;
  const custom = light.mode === CUSTOM_MODE;

  return (
    <section className="glass panel">
      <div className="panel-head">
        <h3>{tr("Iluminação")}</h3>
        <label className="switch">
          <input type="checkbox" checked={light.on} onChange={(e) => update({ ...light, on: e.target.checked })} />
          <span />
        </label>
      </div>
      <div className={light.on ? "light-body" : "light-body off"}>
        <div className="chips">
          {light.modes.map((m) => (
            <button
              key={m.id}
              className={m.id === light.mode ? "chip active" : "chip"}
              onClick={() => update({ ...light, mode: m.id, direction: (EFFECT_CONTROLS[m.id]?.dirs ?? [0])[0] ?? 0 })}
            >
              {tr(m.name)}
            </button>
          ))}
        </div>

        {custom ? (
          <>
            <label className="slider-label">
              <span>{tr("Pincel")}</span>
              <span>{tr("Clique ou arraste nas teclas para pintar")}</span>
            </label>
            <div className="swatches">
              {SWATCHES.map((c) => (
                <button
                  key={toHex(c)}
                  className={toHex(c) === toHex(brush) ? "swatch-btn active" : "swatch-btn"}
                  style={{ background: toHex(c) }}
                  onClick={() => setBrush(c)}
                  title={toHex(c) === "#000000" ? tr("Apagado") : toHex(c)}
                />
              ))}
              <label className="swatch" style={{ background: toHex(brush) }} title={tr("Cor personalizada")}>
                <input type="color" value={toHex(brush)} onChange={(e) => setBrush(fromHex(e.target.value))} />
              </label>
            </div>
            <div className="chips" style={{ marginTop: 10 }}>
              <button className="chip" onClick={() => paint(WIN60.map((k) => k.index))}>
                {tr("Pintar todas")}
              </button>
              <button className="chip" onClick={() => paint(WIN60.map((k) => k.index), [0, 0, 0])}>
                {tr("Apagar todas")}
              </button>
            </div>
          </>
        ) : (
          ctl.color && (
            <div className="color-row">
              <label className="swatch" style={{ background: light.fullColor ? undefined : toHex(light.color) }}>
                <input type="color" value={toHex(light.color)} onChange={(e) => update({ ...light, color: fromHex(e.target.value), fullColor: false })} />
                {light.fullColor && <span className="rainbow-fill" />}
              </label>
              <span className="muted">{light.fullColor ? tr("Arco-íris") : toHex(light.color).toUpperCase()}</span>
              {light.mode !== 0 && (
                <label className="full-color">
                  <input type="checkbox" checked={light.fullColor} onChange={(e) => update({ ...light, fullColor: e.target.checked })} />
                  {tr("Arco-íris")}
                </label>
              )}
            </div>
          )
        )}

        {ctl.dirs.length > 0 && (
          <>
            <label className="slider-label">
              <span>{tr("Direção")}</span>
            </label>
            <div className="chips">
              {ctl.dirs.map((d) => (
                <button
                  key={d}
                  className={light.direction === d ? "chip active" : "chip"}
                  onClick={() => update({ ...light, direction: d })}
                >
                  {tr((rotational ? ROTATION_LABEL : DIRECTION_LABEL)[d])}
                </button>
              ))}
            </div>
          </>
        )}

        <label className="slider-label">
          <span>{tr("Brilho")}</span>
          <span>
            {light.brightness}/{light.maxBrightness}
          </span>
        </label>
        <input
          type="range"
          min={0}
          max={light.maxBrightness}
          value={light.brightness}
          onChange={(e) => update({ ...light, brightness: Number(e.target.value) })}
        />
        {ctl.speed && (
          <>
            <label className="slider-label">
              <span>{tr("Velocidade")}</span>
              <span>
                {light.speed}/{light.maxSpeed}
              </span>
            </label>
            <input
              type="range"
              min={0}
              max={light.maxSpeed}
              value={light.speed}
              onChange={(e) => update({ ...light, speed: Number(e.target.value) })}
            />
          </>
        )}
      </div>
    </section>
  );
}

// ---------------------------------------------------------------- mouse

const MOUSE_EFFECTS: { kind: LedEffect["kind"]; label: string }[] = [
  { kind: "off", label: "Desligado" },
  { kind: "static", label: "Fixo" },
  { kind: "cycle", label: "Ciclo" },
  { kind: "breathe", label: "Respirar" },
];

const DEFAULT_EFFECT: LedEffect = { kind: "cycle", color: [0, 255, 255], periodMs: 5000, brightness: 100 };

/** The mouse can't report its current effect, so remember what we last set (per device + zone). */
function loadEffect(key: string): LedEffect {
  try {
    const raw = localStorage.getItem(key);
    if (raw) return { ...DEFAULT_EFFECT, ...JSON.parse(raw) };
  } catch {
    /* storage unavailable */
  }
  return DEFAULT_EFFECT;
}

function saveEffect(key: string, fx: LedEffect) {
  try {
    localStorage.setItem(key, JSON.stringify(fx));
  } catch {
    /* storage unavailable */
  }
}

export type MouseLightingState = ReturnType<typeof useMouseLighting>;

export function useMouseLighting(id: string, zones: LedZone[] | null, onError: (m: string) => void) {
  const storageKey = (zone: number) => `led:${id}:${zone}`;
  const [effects, setEffects] = useState(() => new Map((zones ?? []).map((z) => [z.index, loadEffect(storageKey(z.index))])));
  const [sync, setSync] = useState(true);
  const push = useDebounced<{ zones: number[]; fx: LedEffect }>(
    async ({ zones: targets, fx }) => {
      for (const z of targets) await setLed(id, z, fx);
    },
    onError,
  );

  const update = (zone: number, fx: LedEffect) => {
    const targets = sync ? (zones ?? []).map((z) => z.index) : [zone];
    const next = new Map(effects);
    targets.forEach((z) => {
      next.set(z, fx);
      saveEffect(storageKey(z), fx);
    });
    setEffects(next);
    push({ zones: targets, fx });
  };

  return { zones: zones ?? [], effects, update, sync, setSync };
}

export function MouseLightingPanel({ state }: { state: MouseLightingState }) {
  const { zones, effects, update, sync, setSync } = state;
  const shown = sync ? zones.slice(0, 1) : zones;

  return (
    <section className="glass panel">
      <div className="panel-head">
        <h3>{tr("Iluminação")}</h3>
        <label className="full-color">
          <input type="checkbox" checked={sync} onChange={(e) => setSync(e.target.checked)} />
          {tr("Sincronizar zonas")}
        </label>
      </div>
      {shown.map((z) => {
        const fx = effects.get(z.index) ?? DEFAULT_EFFECT;
        const set = (patch: Partial<LedEffect>) => update(z.index, { ...fx, ...patch });
        // Period runs 1-20 s; show it as a speed where right = faster.
        const speed = Math.round(((20000 - fx.periodMs) / 19000) * 100);
        return (
          <div key={z.index} className="zone">
            {!sync && <div className="zone-name">{trName(z.name)}</div>}
            <div className="segmented">
              {MOUSE_EFFECTS.filter((e) => z.effects.includes(e.kind)).map((e) => (
                <button key={e.kind} className={fx.kind === e.kind ? "active" : ""} onClick={() => set({ kind: e.kind })}>
                  {tr(e.label)}
                </button>
              ))}
            </div>
            {(fx.kind === "static" || fx.kind === "breathe") && (
              <div className="swatches" style={{ marginTop: 12 }}>
                {SWATCHES.slice(0, 9).map((c) => (
                  <button
                    key={toHex(c)}
                    className={toHex(c) === toHex(fx.color) ? "swatch-btn active" : "swatch-btn"}
                    style={{ background: toHex(c) }}
                    onClick={() => set({ color: c })}
                  />
                ))}
                <label className="swatch" style={{ background: toHex(fx.color) }} title={tr("Cor personalizada")}>
                  <input type="color" value={toHex(fx.color)} onChange={(e) => set({ color: fromHex(e.target.value) })} />
                </label>
              </div>
            )}
            {(fx.kind === "cycle" || fx.kind === "breathe") && (
              <>
                <label className="slider-label">
                  <span>{tr("Velocidade")}</span>
                  <span>{(fx.periodMs / 1000).toFixed(1)} s</span>
                </label>
                <input
                  type="range"
                  min={0}
                  max={100}
                  value={speed}
                  onChange={(e) => set({ periodMs: Math.round(20000 - (Number(e.target.value) / 100) * 19000) })}
                />
                <label className="slider-label">
                  <span>{tr("Brilho")}</span>
                  <span>{fx.brightness}%</span>
                </label>
                <input
                  type="range"
                  min={1}
                  max={100}
                  value={fx.brightness}
                  onChange={(e) => set({ brightness: Number(e.target.value) })}
                />
              </>
            )}
          </div>
        );
      })}
      <p className="hint">
        {tr("O mouse não informa a iluminação atual, então aqui aparece o último efeito definido pelo app.")}
      </p>
    </section>
  );
}

/** Colour to preview on the mouse drawing's logo. */
export function previewColor(fx: LedEffect | undefined): string | "cycle" | null {
  if (!fx || fx.kind === "off") return null;
  if (fx.kind === "cycle") return "cycle";
  return toHex(fx.color);
}
