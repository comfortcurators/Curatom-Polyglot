import { useCallback, useEffect, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import * as api from "@/lib/api";
import { sheetBackdrop, sheetCard, solidBtn } from "@/components/kit";

type Filter = "all" | "intent" | "pattern";

export default function BillboardPlate() {
  const [entries, setEntries] = useState<api.BillboardEntry[]>([]);
  const [filter, setFilter] = useState<Filter>("all");
  const [busy, setBusy] = useState(false);
  const [detail, setDetail] = useState<{ ref: string; body: string } | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setBusy(true);
    try {
      const list = await api.billboard({ kind: filter === "all" ? undefined : filter, limit: 200 });
      setEntries(list);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
    setBusy(false);
  }, [filter]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  async function openBody(ref: string) {
    try {
      const body = await api.billboardBlob(ref);
      setDetail({ ref, body });
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div style={{ height: "100%", overflowY: "auto", padding: 24 }}>
      <h2 style={{ margin: "0 0 16px", fontSize: 22, fontWeight: 700 }}>Billboard</h2>
      <div style={{ display: "flex", gap: 8, marginBottom: 16 }}>
        {(["all", "intent", "pattern"] as const).map((f) => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            style={{
              padding: "6px 12px",
              borderRadius: 6,
              border: "1px solid #333",
              background: filter === f ? "var(--accent)" : "transparent",
              color: filter === f ? "#000" : "var(--ink-faint)",
              fontSize: 12,
              fontWeight: 700,
              cursor: "pointer",
            }}
          >
            {f.toUpperCase()}
          </button>
        ))}
      </div>
      {error ? <p style={{ color: "var(--urgent)" }}>{error}</p> : null}
      {entries.length === 0 && !busy && <p style={{ color: "var(--ink-faint)" }}>No entries yet.</p>}
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        {entries.map((e) => (
          <button
            key={e.id}
            onClick={() => openBody(e.body_ref)}
            style={{
              textAlign: "left",
              border: "1px solid var(--line)",
              borderRadius: 12,
              padding: 14,
              background: "var(--card)",
              cursor: "pointer",
              color: "inherit",
              font: "inherit",
            }}
          >
            <div style={{ display: "flex", justifyContent: "space-between" }}>
              <strong style={{ fontSize: 14 }}>
                {e.kind.toUpperCase()} · {e.key_label || e.key_hash.slice(0, 10)}
              </strong>
              <span style={{ color: "var(--ink-faint)", fontSize: 12 }}>r{e.round}</span>
            </div>
            <p style={{ color: "var(--ink-faint)", fontSize: 12, margin: "4px 0 0" }}>{e.created_at}</p>
            {e.knock_id ? (
              <p style={{ color: "var(--ink-faint)", fontSize: 12, margin: 0 }}>
                knock {e.knock_id.slice(0, 14)}…
              </p>
            ) : null}
          </button>
        ))}
      </div>

      <AnimatePresence>
        {detail && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            style={sheetBackdrop}
            onClick={() => setDetail(null)}
          >
            <motion.div
              initial={{ y: 40, opacity: 0 }}
              animate={{ y: 0, opacity: 1 }}
              exit={{ y: 40, opacity: 0 }}
              style={{ ...sheetCard, maxHeight: "70vh", overflowY: "auto" }}
              onClick={(e) => e.stopPropagation()}
            >
              <h3 style={{ margin: "0 0 4px" }}>Body</h3>
              <p style={{ color: "var(--ink-faint)", fontSize: 12, marginBottom: 12 }}>{detail.ref}</p>
              <pre
                style={{
                  background: "#101012",
                  borderRadius: 12,
                  padding: 14,
                  border: "1px solid #222",
                  color: "#bbb",
                  fontFamily: "monospace",
                  fontSize: 12,
                  lineHeight: 1.5,
                  whiteSpace: "pre-wrap",
                  wordBreak: "break-all",
                }}
              >
                {detail.body}
              </pre>
              <button style={{ ...solidBtn, width: "100%", marginTop: 12 }} onClick={() => setDetail(null)}>
                CLOSE
              </button>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
