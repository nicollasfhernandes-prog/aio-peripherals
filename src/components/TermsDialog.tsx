import { useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "../api";
import { tr } from "../i18n";

// Bump the version to show the dialog again when the terms change.
const KEY = "terms-accepted-v1";

function accepted(): boolean {
  try {
    return localStorage.getItem(KEY) === "yes";
  } catch {
    return false;
  }
}

/** Shown on first launch: use at your own risk, no warranty. Blocks the app until accepted. */
export function TermsDialog() {
  const [open, setOpen] = useState(!accepted());
  if (!open) return null;

  const accept = () => {
    try {
      localStorage.setItem(KEY, "yes");
    } catch {
      /* storage unavailable: ask again next launch */
    }
    setOpen(false);
  };

  return (
    <div className="terms-backdrop" role="dialog" aria-modal="true" aria-labelledby="terms-title">
      <div className="terms-card">
        <h2 id="terms-title">{tr("Termos de uso")}</h2>
        <p>{tr("Este app altera configurações direto no firmware dos seus mouses, teclados e receptores.")}</p>
        <ul>
          <li>
            <strong>{tr("Use por sua conta e risco.")}</strong>{" "}
            {tr("Alguns drivers são experimentais e foram feitos sem o aparelho para testar.")}
          </li>
          <li>
            <strong>{tr("Sem garantia.")}</strong>{" "}
            {tr("O software é fornecido \"no estado em que se encontra\", sem garantia de qualquer tipo.")}
          </li>
          <li>
            {tr("Os autores não se responsabilizam por danos ao hardware, perda de configurações, travamentos ou perda da garantia do fabricante.")}
          </li>
          <li>{tr("Não é afiliado a nenhuma das marcas suportadas.")}</li>
        </ul>
        <p className="muted">
          {tr("O texto completo está no arquivo TERMOS.md do repositório.")}
        </p>
        <div className="terms-actions">
          <button className="terms-btn secondary" onClick={() => (isTauri ? getCurrentWindow().close() : undefined)}>
            {tr("Sair")}
          </button>
          <button className="terms-btn primary" onClick={accept}>
            {tr("Li e aceito")}
          </button>
        </div>
      </div>
    </div>
  );
}
