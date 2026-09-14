import { useCallback, useEffect, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import * as api from "@/lib/api";
import { sheetBackdrop, sheetCard, solidBtn, ghostBtn, card } from "@/components/kit";

export default function KeysPlate() {
  const [keys, setKeys] = useState<api.KeyInfo[]>([]);
  const [creating, setCreating] = useState(false);
  const [detail, setDetail] = useState<api.KeyInfo | null>(null);
  const [lastFile, setLastFile] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setKeys(await api.listKeys());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  async function revoke(token: string) {
    setBusy(true);
    try {
      await api.revokeKey(token);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
    setBusy(false);
  }

  return (
    <div style={{ height: "100%", overflowY: "auto", padding: 24 }}>
      <h2 style={{ margin: "0 0 16px", fontSize: 22, fontWeight: 700 }}>Keys</h2>
      <button style={{ ...solidBtn, width: "100%", marginBottom: 16 }} onClick={() => setCreating(true)}>
        + NEW KEY
      </button>
      {error ? <p style={{ color: "var(--urgent)" }}>{error}</p> : null}
      {keys.length === 0 && (
        <p style={{ color: "var(--ink-faint)" }}>No keys yet. Create one to hand to an AI.</p>
      )}
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        {keys.map((k) => (
          <div key={k.token} style={card}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <strong>{k.label}</strong>
              <span style={{ color: "var(--ink-faint)", fontSize: 12 }}>
                {k.activity_count} activities
              </span>
            </div>
            <p style={{ fontFamily: "monospace", fontSize: 13, color: "#bbb", margin: "6px 0" }}>
              {k.token.slice(0, 24)}…
            </p>
            <p style={{ color: "var(--ink-faint)", fontSize: 12, margin: 0 }}>created {k.created_at}</p>
            {k.last_used_at ? (
              <p style={{ color: "var(--ink-faint)", fontSize: 12, margin: 0 }}>
                last used {k.last_used_at}
              </p>
            ) : null}
            <div style={{ display: "flex", gap: 8, marginTop: 10 }}>
              <button style={ghostBtn} disabled={busy} onClick={() => setDetail(k)}>
                LOG
              </button>
              <button style={ghostBtn} disabled={busy} onClick={() => revoke(k.token)}>
                REVOKE
              </button>
            </div>
          </div>
        ))}
      </div>

      <CreateKeyModal
        visible={creating}
        onClose={() => setCreating(false)}
        onCreated={(fileText) => {
          setLastFile(fileText);
          setCreating(false);
          refresh();
        }}
      />

      <AnimatePresence>
        {lastFile !== null && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            style={sheetBackdrop}
            onClick={() => setLastFile(null)}
          >
            <motion.div
              initial={{ y: 40, opacity: 0 }}
              animate={{ y: 0, opacity: 1 }}
              exit={{ y: 40, opacity: 0 }}
              style={sheetCard}
              onClick={(e) => e.stopPropagation()}
            >
              <h3 style={{ margin: "0 0 12px" }}>Give this to an AI</h3>
              <pre style={codeBox}>{lastFile}</pre>
              <button
                style={{ ...solidBtn, width: "100%" }}
                onClick={async () => {
                  if (lastFile) await navigator.clipboard.writeText(lastFile).catch(() => {});
                  setLastFile(null);
                }}
              >
                COPY & CLOSE
              </button>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

      <KeyLogModal info={detail} onClose={() => setDetail(null)} />
    </div>
  );
}

function CreateKeyModal({
  visible,
  onClose,
  onCreated,
}: {
  visible: boolean;
  onClose: () => void;
  onCreated: (fileText: string) => void;
}) {
  const [label, setLabel] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit() {
    setBusy(true);
    setError(null);
    try {
      const r = await api.createKey(label.trim());
      setLabel("");
      onCreated(r.file_text);
    } catch (e) {
      setError(String(e));
    }
    setBusy(false);
  }

  return (
    <AnimatePresence>
      {visible && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          style={sheetBackdrop}
          onClick={onClose}
        >
          <motion.div
            initial={{ y: 40, opacity: 0 }}
            animate={{ y: 0, opacity: 1 }}
            exit={{ y: 40, opacity: 0 }}
            style={sheetCard}
            onClick={(e) => e.stopPropagation()}
          >
            <h3 style={{ margin: "0 0 8px" }}>New key</h3>
            <p style={{ color: "var(--ink-dim)", fontSize: 13, marginBottom: 12 }}>
              Name it however you like. This label is yours. The AI never sees it.
            </p>
            <input
              value={label}
              onChange={(e) => setLabel(e.target.value)}
              placeholder="e.g. Claude laptop, work key"
              style={inputStyle}
            />
            {error ? <p style={{ color: "var(--urgent)" }}>{error}</p> : null}
            <div style={{ display: "flex", gap: 8, marginTop: 12 }}>
              <button style={ghostBtn} onClick={onClose} disabled={busy}>
                CANCEL
              </button>
              <button
                style={{ ...solidBtn, opacity: busy || !label.trim() ? 0.5 : 1 }}
                onClick={submit}
                disabled={busy || !label.trim()}
              >
                {busy ? "…" : "CREATE"}
              </button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function KeyLogModal({ info, onClose }: { info: api.KeyInfo | null; onClose: () => void }) {
  const [log, setLog] = useState<api.KeyLogEntry[]>([]);
  useEffect(() => {
    if (!info) {
      setLog([]);
      return;
    }
    api.keyLog(info.token).then(setLog).catch(() => setLog([]));
  }, [info]);

  return (
    <AnimatePresence>
      {info && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          style={sheetBackdrop}
          onClick={onClose}
        >
          <motion.div
            initial={{ y: 40, opacity: 0 }}
            animate={{ y: 0, opacity: 1 }}
            exit={{ y: 40, opacity: 0 }}
            style={{ ...sheetCard, maxHeight: "70vh", overflowY: "auto" }}
            onClick={(e) => e.stopPropagation()}
          >
            <h3 style={{ margin: "0 0 4px" }}>{info.label}</h3>
            <p style={{ color: "var(--ink-faint)", fontSize: 12, marginBottom: 12 }}>
              {log.length} entries
            </p>
            {log.map((e, i) => (
              <div key={i} style={{ borderTop: "1px solid var(--line-soft)", padding: "8px 0" }}>
                <strong style={{ fontSize: 14 }}>{e.kind}</strong>
                <p style={{ color: "var(--ink-faint)", fontSize: 12, margin: "2px 0" }}>{e.at}</p>
                {e.name ? (
                  <p style={{ color: "var(--ink-faint)", fontSize: 12, margin: 0 }}>by {e.name}</p>
                ) : null}
                {e.reason ? (
                  <p style={{ color: "var(--ink-faint)", fontSize: 12, margin: 0 }}>{e.reason}</p>
                ) : null}
              </div>
            ))}
            {log.length === 0 && <p style={{ color: "var(--ink-faint)" }}>No activity yet.</p>}
            <button style={{ ...solidBtn, width: "100%", marginTop: 12 }} onClick={onClose}>
              CLOSE
            </button>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

const inputStyle: React.CSSProperties = {
  width: "100%",
  color: "#fff",
  background: "transparent",
  border: "1px solid #333",
  borderRadius: 10,
  padding: 12,
  fontSize: 14,
};

const codeBox: React.CSSProperties = {
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
  marginBottom: 12,
  maxHeight: "40vh",
  overflowY: "auto",
};
