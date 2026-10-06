import type { DeviceKind } from "../api";
import { deviceImage } from "../devices";

/** Top-down render of a symmetric wireless mouse (G Pro Wireless-like). */
/** `logo`: a CSS colour, "cycle" for an animated rainbow, null for off, undefined for the default look. */
export function MouseArt({ size = 220, logo, brand = true }: { size?: number; logo?: string | null; brand?: boolean }) {
  const lit = logo !== undefined && logo !== null;
  return (
    <svg width={size * 0.66} height={size} viewBox="0 0 200 300" className="device-art">
      <defs>
        <radialGradient id="m-body" cx="40%" cy="30%" r="80%">
          <stop offset="0" stopColor="#3a3b3e" />
          <stop offset="0.55" stopColor="#17181a" />
          <stop offset="1" stopColor="#0b0b0c" />
        </radialGradient>
        <linearGradient id="m-wheel" x1="0" x2="1">
          <stop offset="0" stopColor="#1a1a1c" />
          <stop offset="0.5" stopColor="#5a5b5f" />
          <stop offset="1" stopColor="#1a1a1c" />
        </linearGradient>
        <filter id="m-shadow" x="-30%" y="-20%" width="160%" height="150%">
          <feDropShadow dx="0" dy="14" stdDeviation="12" floodColor="#000" floodOpacity="0.55" />
        </filter>
      </defs>
      <path
        filter="url(#m-shadow)"
        fill="url(#m-body)"
        d="M100 8C56 8 34 48 31 112c-3 70 10 160 69 180 59-20 72-110 69-180C166 48 144 8 100 8z"
      />
      {/* button split + button edge */}
      <path d="M100 9v112" stroke="#000" strokeWidth="2" opacity="0.8" />
      <path d="M33 124c30 12 104 12 134 0" stroke="#000" strokeWidth="2" fill="none" opacity="0.6" />
      {/* side highlight */}
      <path
        d="M48 70c-6 30-8 70-4 110"
        stroke="#fff"
        strokeOpacity="0.07"
        strokeWidth="6"
        fill="none"
        strokeLinecap="round"
      />
      {/* wheel */}
      <rect x="91" y="40" width="18" height="44" rx="9" fill="#050506" />
      <rect x="94" y="44" width="12" height="36" rx="6" fill="url(#m-wheel)" />
      {/* DPI LED */}
      <circle cx="100" cy="150" r="2.2" fill="#7dff8a" opacity="0.9" />
      {/* G logo (Logitech only) */}
      {brand && (
      <g
        transform="translate(100 222)"
        fill="none"
        stroke={logo === null ? "#3a3b3e" : lit && logo !== "cycle" ? logo : "#dcdde0"}
        strokeWidth="6"
        strokeLinecap="butt"
        className={logo === "cycle" ? "logo-cycle" : undefined}
        style={lit && logo !== "cycle" ? { filter: `drop-shadow(0 0 6px ${logo})` } : undefined}
      >
        <path d="M14 -12A18 18 0 1 0 18 4H2" />
      </g>
      )}
    </svg>
  );
}

const ROWS: [string, number][][] = [
  [..."`1234567890-=".split("").map((k) => [k, 1] as [string, number]), ["⌫", 2]],
  [["Tab", 1.5], ..."QWERTYUIOP[]".split("").map((k) => [k, 1] as [string, number]), ["\\", 1.5]],
  [["Caps", 1.75], ..."ASDFGHJKL;'".split("").map((k) => [k, 1] as [string, number]), ["Enter", 2.25]],
  [["Shift", 2.25], ..."ZXCVBNM,./".split("").map((k) => [k, 1] as [string, number]), ["Shift", 2.75]],
  [["Ctrl", 1.25], ["Win", 1.25], ["Alt", 1.25], ["", 6.25], ["Alt", 1.25], ["Fn", 1.25], ["Menu", 1.25], ["Ctrl", 1.25]],
];

/** 60% keyboard, generated from the layout above. */
export function KeyboardArt({ width = 300 }: { width?: number }) {
  const u = 20;
  const pad = 8;
  const w = 15 * u + pad * 2;
  const h = 5 * u + pad * 2;
  return (
    <svg width={width} height={(width * h) / w} viewBox={`0 0 ${w} ${h}`} className="device-art">
      <defs>
        <filter id="k-shadow" x="-10%" y="-20%" width="120%" height="160%">
          <feDropShadow dx="0" dy="10" stdDeviation="8" floodColor="#000" floodOpacity="0.55" />
        </filter>
        <linearGradient id="k-cap" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#3a3b3f" />
          <stop offset="1" stopColor="#222326" />
        </linearGradient>
      </defs>
      <rect filter="url(#k-shadow)" x="0" y="0" width={w} height={h} rx="7" fill="#151618" stroke="#2a2b2f" />
      {ROWS.map((row, r) => {
        let x = pad;
        return row.map(([label, units], i) => {
          const kx = x;
          x += units * u;
          return (
            <g key={`${r}-${i}`}>
              <rect x={kx + 1} y={pad + r * u + 1} width={units * u - 2} height={u - 2} rx="3" fill="#0e0f10" />
              <rect x={kx + 2.2} y={pad + r * u + 1.6} width={units * u - 4.4} height={u - 5} rx="2.5" fill="url(#k-cap)" />
              {label && (
                <text
                  x={kx + 4.5}
                  y={pad + r * u + 8.5}
                  fontSize={label.length > 1 ? 4.2 : 5.5}
                  fill="#c9cacd"
                  fontFamily="Inter, system-ui, sans-serif"
                >
                  {label}
                </text>
              )}
            </g>
          );
        });
      })}
    </svg>
  );
}

interface ArtProps {
  kind: DeviceKind;
  large?: boolean;
  logo?: string | null;
  vendor?: string;
  name?: string;
}

export function DeviceArt({ kind, large = false, logo, vendor = "Logitech", name = "" }: ArtProps) {
  const photo = kind === "mouse" ? deviceImage(vendor, name) : undefined;
  if (photo) {
    return <img src={photo} alt={name} className="device-art device-photo" style={{ height: large ? 340 : 210 }} />;
  }
  return kind === "mouse" ? (
    <MouseArt size={large ? 340 : 210} logo={logo} brand={vendor === "Logitech"} />
  ) : (
    <KeyboardArt width={large ? 520 : 290} />
  );
}
