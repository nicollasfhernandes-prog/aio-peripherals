// Device catalog: real top-down product photos, matched by vendor + name.
import deluxM900 from "./assets/devices/delux-m900-pro.png";
import haste2 from "./assets/devices/hyperx-pulsefire-haste-2.png";
import gproWireless from "./assets/devices/logitech-g-pro-wireless.png";

interface CatalogEntry {
  vendor: string;
  match: RegExp;
  image: string;
}

const CATALOG: CatalogEntry[] = [
  { vendor: "Logitech", match: /g pro wireless/i, image: gproWireless },
  { vendor: "HyperX", match: /haste 2/i, image: haste2 },
  { vendor: "DeLUX", match: /m900/i, image: deluxM900 },
];

export function deviceImage(vendor: string, name: string): string | undefined {
  return CATALOG.find((e) => e.vendor === vendor && e.match.test(name))?.image;
}
