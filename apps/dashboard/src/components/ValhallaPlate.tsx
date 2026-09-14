import { useCallback, useEffect, useState } from "react";
import * as api from "@/lib/api";
import { card } from "@/components/kit";

export default function ValhallaPlate() {
  const [sessions, setSessions] = useState<api.ValhallaSession[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setBusy(true);
    try {
      setSessions(await api.valhallaSessions());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
    setBusy(false);
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  return (
    <div style={{ height: "100%", overflowY: "auto", padding: 24 }}>
      <h2 style={{ margin: "0 0 16px", fontSize: 22, fontWeight: 700 }}>Valhalla</h2>
      {error ? <p style={{ color: "var(--urgent)" }}>{error}</p> : null}
      {sessions.length === 0 && !busy && (
        <p style={{ color: "var(--ink-faint)" }}>No Valhalla sessions.</p>
      )}
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        {sessions.map((v) => (
          <div key={v.sandbox_id} style={card}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <strong>{v.label}</strong>
              <span
                style={{
                  fontSize: 12,
                  fontWeight: 700,
                  color: v.alive ? "var(--ok)" : "var(--urgent)",
                }}
              >
                {v.alive ? "live" : "closed"}
              </span>
            </div>
            <p style={{ fontFamily: "monospace", fontSize: 13, color: "#bbb", margin: "6px 0" }}>
              {v.sandbox_id}
            </p>
            <p style={{ color: "var(--ink-faint)", fontSize: 12, margin: 0 }}>opened {v.opened_at}</p>
            {v.closed_at ? (
              <p style={{ color: "var(--ink-faint)", fontSize: 12, margin: 0 }}>closed {v.closed_at}</p>
            ) : null}
            <p style={{ color: "var(--ink-faint)", fontSize: 12, margin: 0 }}>
              {v.entry_count} actions inside
            </p>
          </div>
        ))}
      </div>
    </div>
  );
}
