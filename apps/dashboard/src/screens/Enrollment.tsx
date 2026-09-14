import { useState } from "react";
import { motion } from "framer-motion";
import * as api from "@/lib/api";

export default function Enrollment({ onEnrolled }: { onEnrolled: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function begin() {
    setBusy(true);
    setError(null);
    try {
      await api.claim();
      onEnrolled();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }

  return (
    <div
      style={{
        height: "100%",
        display: "flex",
        flexDirection: "column",
        justifyContent: "center",
        padding: 24,
        maxWidth: 480,
        margin: "0 auto",
      }}
    >
      <motion.h1
        initial={{ opacity: 0, y: 8 }}
        animate={{ opacity: 1, y: 0 }}
        style={{ fontSize: 28, fontWeight: 700, marginBottom: 16 }}
      >
        You are the operator.
      </motion.h1>
      <p style={{ color: "var(--ink-dim)", fontSize: 16, lineHeight: 1.6, marginBottom: 16 }}>
        Curatom is where machines ask you for things. You say yes or no. Nothing else.
      </p>
      <p style={{ color: "var(--ink-dim)", fontSize: 16, lineHeight: 1.6, marginBottom: 24 }}>
        You will get a token. Give it to any AI you trust. When they come to Curatom with
        it, you will see their knock and decide.
      </p>
      {error ? <p style={{ color: "var(--urgent)", marginBottom: 16 }}>{error}</p> : null}
      <motion.button
        whileTap={{ scale: 0.97 }}
        onClick={begin}
        disabled={busy}
        style={{
          background: "var(--accent)",
          color: "#000",
          fontWeight: 700,
          padding: "16px 20px",
          borderRadius: 10,
          border: "none",
          opacity: busy ? 0.5 : 1,
          cursor: busy ? "default" : "pointer",
        }}
      >
        {busy ? "…" : "BEGIN"}
      </motion.button>
    </div>
  );
}
