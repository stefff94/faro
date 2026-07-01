import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export function FirstRunConsent({ onDone }: { onDone: () => void }) {
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  async function activate() {
    setBusy(true);
    setErr(null);
    try {
      await invoke("faro_register_hooks");
      onDone();
    } catch (e) {
      setErr(String(e));
      setBusy(false);
    }
  }

  return (
    <div className="consent">
      <div className="consent-title">Faro</div>
      <div className="consent-body">
        Registro gli hook di Claude Code per monitorare le sessioni.
        Nessun comando da lanciare.
      </div>
      {err && <div className="consent-err">{err}</div>}
      <div className="consent-actions">
        <button className="consent-primary" onClick={activate} disabled={busy}>
          {busy ? "Attivo…" : "Attiva"}
        </button>
        <button className="consent-later" onClick={onDone} disabled={busy}>
          Più tardi
        </button>
      </div>
    </div>
  );
}
