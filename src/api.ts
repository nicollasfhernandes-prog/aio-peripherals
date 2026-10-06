import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { WIN60 } from "./layouts/win60";

export type DeviceKind = "mouse" | "keyboard";

export interface Lighting {
  on: boolean;
  mode: number;
  brightness: number;
  speed: number;
  color: [number, number, number];
  background: [number, number, number];
  direction: number;
  fullColor: boolean;
  modes: { id: number; name: string }[];
  maxBrightness: number;
  maxSpeed: number;
  /** Per-key colours used by the Custom effect (mode 10) */
  custom: { index: number; color: [number, number, number] }[];
}

export interface LedZone {
  index: number;
  name: string;
  effects: string[];
}

export interface LedEffect {
  kind: "off" | "static" | "cycle" | "breathe";
  color: [number, number, number];
  periodMs: number;
  /** 1-100 */
  brightness: number;
}

/** Trigger values are in device units; multiply by unitMm for millimetres. */
export interface Trigger {
  /** 0 = fixed actuation, 12 = rapid trigger, 13 = rapid trigger with separate release */
  mode: number;
  travel: number;
  press: number;
  release: number;
}

export interface Actuation {
  min: number;
  max: number;
  unitMm: number;
  keys: (Trigger & { index: number })[];
}

export interface SensorSettings {
  angleSnap: boolean | null;
  rippleControl: boolean | null;
  motionSync: boolean | null;
  lod: number | null;
  lodOptions: [number, string][];
  debounceMs: number | null;
  debounceMax: number;
}

export interface Device {
  id: string;
  name: string;
  vendor: string;
  kind: DeviceKind;
  connection: string;
  online: boolean;
  supported: boolean;
  battery: { percent: number; charging: boolean; estimated: boolean } | null;
  dpi: { current: number; min: number; max: number; step: number } | null;
  reportRate: { currentHz: number; supportedHz: number[] } | null;
  onboardMode: boolean | null;
  note: string | null;
  keyboard: { lighting: Lighting | null; actuation: Actuation | null } | null;
  ledZones: LedZone[] | null;
  sensor: SensorSettings | null;
}

export const isTauri = "__TAURI_INTERNALS__" in window;

// Lets the UI run in a plain browser (npm run dev) without hardware.
let mock: Device[] = [
  {
    id: "logitech:c539:1",
    name: "G Pro Wireless Gaming Mouse",
    vendor: "Logitech",
    kind: "mouse",
    connection: "Lightspeed",
    online: true,
    supported: true,
    battery: { percent: 100, charging: false, estimated: true },
    dpi: { current: 1600, min: 100, max: 25600, step: 50 },
    reportRate: { currentHz: 1000, supportedHz: [125, 250, 500, 1000] },
    onboardMode: true,
    note: null,
    keyboard: null,
    ledZones: [
      { index: 0, name: "Principal", effects: ["off", "static", "cycle", "breathe"] },
      { index: 1, name: "Logo", effects: ["off", "static", "cycle", "breathe"] },
    ],
    sensor: null,
  },
  {
    id: "compx:fb26",
    name: "DeLUX M900",
    vendor: "DeLUX",
    kind: "mouse",
    connection: "2.4 GHz",
    online: true,
    supported: true,
    battery: { percent: 82, charging: false, estimated: false },
    dpi: { current: 800, min: 50, max: 26000, step: 50 },
    reportRate: { currentHz: 1000, supportedHz: [125, 250, 500, 1000, 2000, 4000, 8000] },
    onboardMode: null,
    note: null,
    keyboard: null,
    ledZones: [{ index: 0, name: "LED de DPI", effects: ["off", "static", "breathe"] }],
    sensor: {
      angleSnap: false,
      rippleControl: false,
      motionSync: true,
      lod: 1,
      lodOptions: [[1, "1 mm"], [2, "2 mm"]],
      debounceMs: 4,
      debounceMax: 15,
    },
  },
  {
    id: "aula:c365",
    name: "WIN 60 HE",
    vendor: "AULA",
    kind: "keyboard",
    connection: "USB",
    online: true,
    supported: true,
    battery: null,
    dpi: null,
    reportRate: { currentHz: 4000, supportedHz: [1000, 2000, 4000, 8000] },
    onboardMode: null,
    note: null,
    ledZones: null,
    sensor: null,
    keyboard: {
      lighting: {
        on: true,
        mode: 0,
        brightness: 4,
        speed: 0,
        color: [0, 255, 255],
        background: [0, 0, 0],
        direction: 0,
        fullColor: false,
        modes: [
          [2, "Onda"], [1, "Respirar"], [3, "Neon"], [9, "Cintilante"], [4, "Radar"], [0, "Fixo"],
          [14, "Onda automática"], [15, "Listras"], [6, "Reativo"], [7, "Aurora"], [11, "Cruz"],
          [12, "Resposta rápida"], [16, "Fogos"], [8, "Marola"], [10, "Personalizado"],
        ].map(([id, name]) => ({ id: id as number, name: name as string })),
        maxBrightness: 4,
        maxSpeed: 4,
        custom: WIN60.map((k) => ({ index: k.index, color: [0, 0, 0] as [number, number, number] })),
      },
      actuation: {
        min: 8,
        max: 340,
        unitMm: 0.01,
        keys: WIN60.map((k) => ({ index: k.index, mode: 0, travel: 39, press: 98, release: 98 })),
      },
    },
  },
];

function patchMock(id: string, f: (d: Device) => Device) {
  mock = mock.map((d) => (d.id === id ? f(d) : d));
}

export async function listDevices(): Promise<Device[]> {
  if (!isTauri) return structuredClone(mock);
  return invoke("list_devices");
}

export async function setDpi(id: string, dpi: number): Promise<void> {
  if (!isTauri) return patchMock(id, (d) => ({ ...d, dpi: d.dpi && { ...d.dpi, current: dpi } }));
  return invoke("set_dpi", { id, dpi });
}

/** Resolves true when the device is about to reconnect (AULA reboots its USB on a rate change). */
export async function setReportRate(id: string, hz: number): Promise<boolean> {
  if (!isTauri) {
    patchMock(id, (d) => ({
      ...d,
      onboardMode: d.onboardMode === null ? null : false,
      reportRate: d.reportRate && { ...d.reportRate, currentHz: hz },
    }));
    return false;
  }
  return invoke("set_report_rate", { id, hz });
}

export async function setLighting(id: string, lighting: Lighting): Promise<void> {
  if (!isTauri) return patchMock(id, (d) => ({ ...d, keyboard: d.keyboard && { ...d.keyboard, lighting } }));
  return invoke("set_lighting", { id, lighting });
}

/** Applies `trigger` to the given key matrix indices (empty = every key). */
export async function setTrigger(id: string, trigger: Trigger, keys: number[]): Promise<void> {
  if (!isTauri)
    return patchMock(id, (d) => {
      const a = d.keyboard?.actuation;
      if (!d.keyboard || !a) return d;
      const keysOut = a.keys.map((k) => (keys.length === 0 || keys.includes(k.index) ? { ...k, ...trigger } : k));
      return { ...d, keyboard: { ...d.keyboard, actuation: { ...a, keys: keysOut } } };
    });
  return invoke("set_trigger", { id, trigger, keys });
}

export async function setCustomColors(id: string, colors: { index: number; color: number[] }[]): Promise<void> {
  if (!isTauri)
    return patchMock(id, (d) => {
      const l = d.keyboard?.lighting;
      if (!d.keyboard || !l) return d;
      const custom = colors.map((c) => ({ index: c.index, color: c.color as [number, number, number] }));
      return { ...d, keyboard: { ...d.keyboard, lighting: { ...l, custom } } };
    });
  return invoke("set_custom_colors", { id, colors });
}

export async function setLed(id: string, zone: number, effect: LedEffect): Promise<void> {
  if (!isTauri) return;
  return invoke("set_led", { id, zone, effect });
}

export async function setSensor(id: string, key: string, value: number): Promise<void> {
  if (!isTauri) return;
  return invoke("set_sensor", { id, key, value });
}

export async function setOnboardMode(id: string, onboard: boolean): Promise<void> {
  if (!isTauri) return patchMock(id, (d) => ({ ...d, onboardMode: onboard }));
  return invoke("set_onboard_mode", { id, onboard });
}

/**
 * Streams live key travel (device units) as [matrix index, travel] batches.
 * Returns a function that stops the stream.
 */
export async function startKeyMonitor(id: string, onBatch: (batch: [number, number][]) => void): Promise<() => void> {
  if (!isTauri) {
    // Fake a few keys being pressed so the UI can be developed without hardware.
    const demo = [46, 68, 69, 70];
    let t = 0;
    const timer = window.setInterval(() => {
      t += 0.05;
      onBatch(demo.map((k, i) => [k, Math.round(Math.max(0, Math.sin(t * 2 + i)) * 340)]));
    }, 16);
    return () => window.clearInterval(timer);
  }
  const unlisten = await listen<[number, number][]>("key-travel", (e) => onBatch(e.payload));
  try {
    await invoke("start_key_monitor", { id });
  } catch (e) {
    unlisten();
    throw e;
  }
  return () => {
    unlisten();
    invoke("stop_key_monitor").catch(() => {});
  };
}
