import { useCallback, useEffect, useState } from "react";
import * as api from "@/lib/api";
import Enrollment from "@/screens/Enrollment";
import Dashboard from "@/components/Dashboard";

export default function App() {
  const [me, setMe] = useState<api.Me | null>(null);
  const [bootError, setBootError] = useState<string | null>(null);

  const refreshMe = useCallback(async () => {
    try {
      setMe(await api.me());
      setBootError(null);
    } catch (e) {
      setBootError(String(e));
    }
  }, []);

  useEffect(() => {
    refreshMe();
  }, [refreshMe]);

  if (bootError) {
    return (
      <div style={centerStyle}>
        <p style={{ color: "var(--urgent)", fontWeight: 700 }}>Could not reach Curatom.</p>
        <p style={{ color: "var(--ink-faint)" }}>{bootError}</p>
        <button
          onClick={refreshMe}
          style={{
            background: "var(--accent)",
            color: "#000",
            fontWeight: 700,
            padding: "10px 18px",
            borderRadius: 10,
            border: "none",
            cursor: "pointer",
          }}
        >
          RETRY
        </button>
      </div>
    );
  }

  if (!me) {
    return <div style={centerStyle}>…</div>;
  }

  if (!me.enrolled) {
    return <Enrollment onEnrolled={refreshMe} />;
  }

  return <Dashboard />;
}

const centerStyle: React.CSSProperties = {
  height: "100%",
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  justifyContent: "center",
  gap: 12,
  padding: 24,
};
