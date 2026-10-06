import { tr } from "../i18n";
import { useEffect, useRef, useState } from "react";
import { setDpi, setOnboardMode, setReportRate, type Device } from "../api";
import { DeviceArt } from "./DeviceArt";
import {
  ActuationPanel,
  InteractiveKeyboard,
  LivePanel,
  useKeyEditor,
  useLiveTravel,
  type KeyboardView,
} from "./KeyboardPanels";
import {
  KeyboardLightingPanel,
  MouseLightingPanel,
  previewColor,
  useKeyboardLighting,
  useMouseLighting,
} from "./Lighting";
import { SensorPanel } from "./SensorPanel";
import { BackIcon, BatteryIcon, BoltIcon } from "./Icons";

const DPI_PRESETS = [400, 800, 1600, 3200];

interface Props {
  device: Device;
  onBack: () => void;
  onChanged: () => void;
  onError: (msg: string) => void;
}

export function DeviceDetail({ device, onBack, onChanged, onError }: Props) {
  const [dpi, setDpiLocal] = useState(device.dpi?.current ?? 0);
  const timer = useRef<number>(undefined);

  useEffect(() => setDpiLocal(device.dpi?.current ?? 0), [device.dpi?.current]);

  const actuation = device.keyboard?.actuation ?? null;
  const lighting = device.keyboard?.lighting ?? null;
  const keyEditor = useKeyEditor(device.id, actuation, onError);
  const [view, setView] = useState<KeyboardView>("actuation");
  const live = useLiveTravel(device.id, view === "live" && !!actuation, onError);
  const kbLighting = useKeyboardLighting(device.id, lighting, onError);
  const mouseLighting = useMouseLighting(device.id, device.ledZones, onError);
  const logo = previewColor(mouseLighting.effects.get(1) ?? mouseLighting.effects.get(0));

  const run = async (p: Promise<void>, reload = true) => {
    try {
      await p;
      if (reload) onChanged();
    } catch (e) {
      onError(String(e));
    }
  };

  const [reconnecting, setReconnecting] = useState(false);
  const changeRate = async (hz: number) => {
    if (await setReportRate(device.id, hz)) {
      // The keyboard reboots its USB connection to apply a new rate.
      setReconnecting(true);
      await new Promise((r) => setTimeout(r, 3000));
      setReconnecting(false);
    }
  };

  const commitDpi = (value: number) => {
    setDpiLocal(value);
    window.clearTimeout(timer.current);
    // The slider already shows the new value, so skip the (slow) full device rescan.
    timer.current = window.setTimeout(() => run(setDpi(device.id, value), false), 120);
  };

  const { dpi: dpiInfo, reportRate, battery } = device;

  return (
    <div className="detail">
      <button className="back" onClick={onBack}>
        <BackIcon /> Devices
      </button>

      <div className={actuation ? "detail-grid wide-hero" : "detail-grid"}>
        <section className="glass detail-hero">
          <header>
            <h2>{device.name}</h2>
            <span className="muted">
              {device.vendor} · {device.connection}
            </span>
          </header>
          <div className="hero-art">
            {actuation ? (
              <InteractiveKeyboard
                editor={keyEditor}
                actuation={actuation}
                live={live}
                lighting={kbLighting}
                view={view}
                onViewChange={setView}
              />
            ) : (
              <DeviceArt kind={device.kind} large logo={device.ledZones ? logo : undefined} vendor={device.vendor} name={device.name} />
            )}
          </div>
        </section>

        <div className="panels">
          {!device.supported && (
            <section className="glass panel">
              <h3>{tr(device.online ? "Em breve" : "Offline")}</h3>
              <p className="muted">{device.note && tr(device.note)}</p>
            </section>
          )}

          {actuation && view === "live" && live && <LivePanel live={live} editor={keyEditor} actuation={actuation} />}
          {actuation && view === "actuation" && <ActuationPanel editor={keyEditor} actuation={actuation} />}
          {lighting && (view === "lighting" || !actuation) && <KeyboardLightingPanel state={kbLighting} />}
          {device.ledZones && device.ledZones.length > 0 && <MouseLightingPanel state={mouseLighting} />}

          {battery && (
            <section className="glass panel">
              <h3>{tr("Bateria")}</h3>
              <div className="battery-row">
                <BatteryIcon level={battery.percent / 100} width={28} height={28} />
                <span className="big">
                  {battery.estimated && "~"}
                  {battery.percent}%
                </span>
                {battery.charging && (
                  <span className="pill">
                    <BoltIcon width={12} height={12} /> {tr("Carregando")}
                  </span>
                )}
              </div>
              <div className="bar">
                <div style={{ width: `${battery.percent}%` }} />
              </div>
            </section>
          )}

          {dpiInfo && (
            <section className="glass panel">
              <div className="panel-head">
                <h3>{tr("Sensibilidade")}</h3>
                <span className="big">
                  {dpi} <small>DPI</small>
                </span>
              </div>
              <input
                type="range"
                min={dpiInfo.min}
                max={Math.min(dpiInfo.max, 6400)}
                step={dpiInfo.step}
                value={dpi}
                onChange={(e) => commitDpi(Number(e.target.value))}
              />
              <div className="chips">
                {DPI_PRESETS.filter((p) => p >= dpiInfo.min && p <= dpiInfo.max).map((p) => (
                  <button key={p} className={p === dpi ? "chip active" : "chip"} onClick={() => commitDpi(p)}>
                    {p}
                  </button>
                ))}
                <input
                  className="chip-input"
                  type="number"
                  min={dpiInfo.min}
                  max={dpiInfo.max}
                  step={dpiInfo.step}
                  value={dpi}
                  onChange={(e) => setDpiLocal(Number(e.target.value))}
                  onBlur={(e) => commitDpi(clampDpi(Number(e.target.value), dpiInfo))}
                  onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
                />
              </div>
              <p className="hint">
                {tr("Faixa de {min} a {max}, em passos de {step}.", { min: dpiInfo.min, max: dpiInfo.max, step: dpiInfo.step })}
              </p>
            </section>
          )}

          {reportRate && (
            <section className="glass panel">
              <div className="panel-head">
                <h3>{tr("Taxa de polling")}</h3>
                {reconnecting && <span className="muted">{tr("Reconectando…")}</span>}
              </div>
              <div className="segmented">
                {reportRate.supportedHz.map((hz) => (
                  <button
                    key={hz}
                    className={hz === reportRate.currentHz ? "active" : ""}
                    onClick={() => run(changeRate(hz))}
                  >
                    {hz} Hz
                  </button>
                ))}
              </div>
            </section>
          )}

          {device.sensor && <SensorPanel key={device.id} id={device.id} sensor={device.sensor} onError={onError} />}

          {device.onboardMode !== null && (
            <section className="glass panel">
              <div className="panel-head">
                <h3>{tr("Memória interna")}</h3>
                <label className="switch">
                  <input
                    type="checkbox"
                    checked={device.onboardMode}
                    onChange={(e) => run(setOnboardMode(device.id, e.target.checked))}
                  />
                  <span />
                </label>
              </div>
              <p className="hint">
                {device.onboardMode
                  ? tr("O mouse usa o perfil salvo na memória dele. Mudar o polling ou a iluminação passa para o modo software.")
                  : tr("Modo software: as configurações vêm deste app e se perdem quando o mouse desliga. Ative para voltar ao perfil salvo.")}
              </p>
            </section>
          )}
        </div>
      </div>
    </div>
  );
}

function clampDpi(v: number, info: NonNullable<Device["dpi"]>) {
  const snapped = Math.round(v / info.step) * info.step;
  return Math.min(info.max, Math.max(info.min, snapped || info.min));
}
