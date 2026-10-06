import { useCallback, useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri, listDevices, type Device } from "./api";
import { deviceImage } from "./devices";
import { getLang, setLang, tr, type Lang } from "./i18n";
import { DeviceDetail } from "./components/DeviceDetail";
import { TermsDialog } from "./components/TermsDialog";
import { BatteryIcon, BoltIcon, KeyboardIcon, MouseIcon, RefreshIcon, UsbIcon, WinClose, WinMax, WinMin, WirelessIcon } from "./components/Icons";
import "./App.css";

interface TitleBarProps {
  loading: boolean;
  onRefresh: () => void;
  lang: Lang;
  onLang: (l: Lang) => void;
}

function TitleBar({ loading, onRefresh, lang, onLang }: TitleBarProps) {
  const win = isTauri ? getCurrentWindow() : null;
  return (
    <header className="titlebar" data-tauri-drag-region>
      <div className="brand" data-tauri-drag-region>
        <span className="brand-mark">AIO</span>
        <span className="brand-name" data-tauri-drag-region>
          {tr("Periféricos")}
        </span>
        <span className="brand-tag" data-tauri-drag-region>
          alpha
        </span>
      </div>
      <div className="titlebar-actions">
        <div className="lang-switch" role="group" aria-label={tr("Idioma")}>
          {(["pt", "en"] as Lang[]).map((l) => (
            <button key={l} className={lang === l ? "active" : ""} onClick={() => onLang(l)}>
              {l.toUpperCase()}
            </button>
          ))}
        </div>
        <button className={loading ? "icon-btn spinning" : "icon-btn"} onClick={onRefresh} title={tr("Procurar dispositivos")}>
          <RefreshIcon />
        </button>
      </div>
      <div className="win-controls">
        <button onClick={() => win?.minimize()} aria-label={tr("Minimizar")}>
          <WinMin />
        </button>
        <button onClick={() => win?.toggleMaximize()} aria-label={tr("Maximizar")}>
          <WinMax />
        </button>
        <button className="close" onClick={() => win?.close()} aria-label={tr("Fechar")}>
          <WinClose />
        </button>
      </div>
    </header>
  );
}

function statusOf(d: Device): { label: string; tone: "ok" | "warn" | "muted" } {
  if (!d.online) return { label: tr("dormindo"), tone: "warn" };
  if (!d.supported) return { label: tr("só detecção"), tone: "muted" };
  return { label: tr("conectado"), tone: "ok" };
}

function DeviceRow({ device, active, onSelect }: { device: Device; active: boolean; onSelect: () => void }) {
  const photo = deviceImage(device.vendor, device.name);
  const status = statusOf(device);
  return (
    <button className={active ? "device-row active" : "device-row"} onClick={onSelect}>
      <span className="device-thumb">
        {photo ? (
          <img src={photo} alt="" />
        ) : device.kind === "keyboard" ? (
          <KeyboardIcon width={22} height={22} />
        ) : (
          <MouseIcon width={22} height={22} />
        )}
      </span>
      <span className="device-meta">
        <span className="device-name">{device.name}</span>
        <span className="device-sub">
          {device.vendor}
          <span className="sep">·</span>
          {device.connection === "USB" ? <UsbIcon width={11} height={11} /> : <WirelessIcon width={11} height={11} />}
          {device.connection}
        </span>
      </span>
      <span className="device-side">
        {device.battery && (
          <span className="battery-chip">
            {device.battery.charging ? <BoltIcon width={11} height={11} /> : <BatteryIcon width={13} height={13} level={device.battery.percent / 100} />}
            {device.battery.percent}%
          </span>
        )}
        <span className={`status-dot ${status.tone}`} title={status.label} />
      </span>
    </button>
  );
}

export default function App() {
  const [devices, setDevices] = useState<Device[]>([]);
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [lang, setLangState] = useState<Lang>(getLang());
  const changeLang = (l: Lang) => {
    setLang(l);
    setLangState(l); // re-renders the whole tree with the new language
  };

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      setDevices(await listDevices());
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  // Rescan often while a wireless mouse is asleep/unreachable, so it shows up as soon as it wakes.
  const anyOffline = devices.some((d) => !d.online || (d.kind === "mouse" && !d.supported));
  useEffect(() => {
    const t = window.setInterval(refresh, anyOffline ? 5000 : 30000);
    return () => window.clearInterval(t);
  }, [refresh, anyOffline]);

  useEffect(() => {
    if (!error) return;
    const t = window.setTimeout(() => setError(null), 5000);
    return () => window.clearTimeout(t);
  }, [error]);

  // Always show something: fall back to the first device.
  const current = devices.find((d) => d.id === selected) ?? devices[0];
  const mice = devices.filter((d) => d.kind === "mouse");
  const keyboards = devices.filter((d) => d.kind === "keyboard");

  return (
    <div className="app">
      <TitleBar loading={loading} onRefresh={refresh} lang={lang} onLang={changeLang} />
      <div className="layout">
        <aside className="rail">
          {[
            ["Mouses", mice],
            ["Teclados", keyboards],
          ].map(([title, list]) =>
            (list as Device[]).length === 0 ? null : (
              <section key={title as string} className="rail-group">
                <h2 className="rail-title">
                  {tr(title as string)} <span className="count">{(list as Device[]).length}</span>
                </h2>
                {(list as Device[]).map((d) => (
                  <DeviceRow key={d.id} device={d} active={current?.id === d.id} onSelect={() => setSelected(d.id)} />
                ))}
              </section>
            ),
          )}
          {devices.length === 0 && !loading && (
            <p className="rail-empty">{tr("Nenhum dispositivo encontrado. Conecte algo e clique em atualizar.")}</p>
          )}
          <div className="rail-footer">
            {devices.length} {lang === "en" ? (devices.length === 1 ? "device" : "devices") : devices.length === 1 ? "dispositivo" : "dispositivos"}
          </div>
        </aside>

        <main className="content">
          {current ? (
            <DeviceDetail key={current.id} device={current} onBack={() => setSelected(null)} onChanged={refresh} onError={setError} />
          ) : (
            <div className="content-empty">{tr(loading ? "Procurando dispositivos…" : "Selecione um dispositivo")}</div>
          )}
        </main>
      </div>
      {error && <div className="toast">{tr(error)}</div>}
      <TermsDialog />
    </div>
  );
}
