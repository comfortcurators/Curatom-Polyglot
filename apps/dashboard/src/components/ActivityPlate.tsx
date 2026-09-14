import { useCallback, useEffect, useState } from "react";
import * as api from "@/lib/api";

export default function ActivityPlate() {
  const [events, setEvents] = useState<{ kind: string; at: string; actor: string }[]>([]);

  const refresh = useCallback(async () => {
    try {
      setEvents(await api.activity());
    } catch {
      // best-effort feed
    }
  }, []);

  useEffect(() => {
    refresh();
    const iv = setInterval(refresh, 4000);
    return () => clearInterval(iv);
  }, [refresh]);

  return (
    <div style={{ height: "100%", overflowY: "auto", padding: 24 }}>
      <h2 style={{ margin: "0 0 16px", fontSize: 22, fontWeight: 700 }}>Activity</h2>
      {events.length === 0 && <p style={{ color: "var(--ink-faint)" }}>Nothing yet.</p>}
      <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        {events
          .slice()
          .reverse()
          .map((e, i) => (
            <div
              key={i}
              style={{
                display: "flex",
                justifyContent: "space-between",
                borderBottom: "1px solid var(--line-soft)",
                padding: "8px 0",
                fontSize: 13,
              }}
            >
              <span>{e.kind}</span>
              <span style={{ color: "var(--ink-faint)" }}>{e.at}</span>
            </div>
          ))}
      </div>
    </div>
  );
}
