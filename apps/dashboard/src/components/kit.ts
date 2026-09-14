import type { CSSProperties } from "react";

export const solidBtn: CSSProperties = {
  background: "var(--accent)",
  color: "#000",
  fontWeight: 700,
  padding: 12,
  borderRadius: 10,
  border: "none",
  cursor: "pointer",
  flex: 1,
};

export const ghostBtn: CSSProperties = {
  background: "transparent",
  color: "#ccc",
  fontWeight: 700,
  padding: 12,
  borderRadius: 10,
  border: "1px solid #444",
  cursor: "pointer",
  flex: 1,
};

export const card: CSSProperties = {
  border: "1px solid var(--line)",
  borderRadius: 12,
  padding: 14,
  background: "var(--card)",
};

export const sheetBackdrop: CSSProperties = {
  position: "fixed",
  inset: 0,
  background: "rgba(0,0,0,0.75)",
  display: "flex",
  alignItems: "flex-end",
  justifyContent: "center",
  zIndex: 100,
};

export const sheetCard: CSSProperties = {
  background: "var(--panel)",
  borderRadius: "18px 18px 0 0",
  padding: 24,
  width: "100%",
  maxWidth: 480,
};
