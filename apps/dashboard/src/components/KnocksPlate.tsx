import { useCallback, useEffect, useState } from "react";
import { AnimatePresence, motion, useMotionValue, useTransform } from "framer-motion";
import * as api from "@/lib/api";
import TurnstileGate from "@/components/TurnstileGate";

const SWIPE_THRESHOLD = 120;
const SWIPE_VELOCITY = 500;

export default function KnocksPlate() {
  const [knocks, setKnocks] = useState<api.Knock[]>([]);
  const [busy, setBusy] = useState(false);
  const [toast, setToast] = useState("");
  const [confirming, setConfirming] = useState<api.Knock | null>(null);
  const [tsToken, setTsToken] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setKnocks(await api.listKnocks());
    } catch (e) {
      setToast(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
    const iv = setInterval(refresh, 2000);
    return () => clearInterval(iv);
  }, [refresh]);

  async function doRefuse(id: string) {
    setKnocks((cur) => cur.filter((k) => k.id !== id));
    setBusy(true);
    try {
      await api.refuseKnock(id);
      setToast("Refused.");
    } catch (e) {
      setToast(String(e));
    }
    setBusy(false);
    refresh();
  }

  async function doApprove() {
    if (!confirming || !tsToken) return;
    const id = confirming.id;
    const token = tsToken;
    setKnocks((cur) => cur.filter((k) => k.id !== id));
    setConfirming(null);
    setTsToken(null);
    setBusy(true);
    try {
      await api.approveKnock(id, token);
      setToast("Approved.");
    } catch (e) {
      setToast(String(e));
    }
    setBusy(false);
    refresh();
  }

  const top3 = knocks.slice(0, 3);

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column", padding: 24 }}>
      <h2 style={{ margin: "0 0 4px", fontSize: 22, fontWeight: 700 }}>Knocks</h2>
      <p style={{ color: "var(--ink-faint)", fontSize: 13, margin: "0 0 20px" }}>
        Swipe right to approve, left to refuse. Or use the buttons.
      </p>
      {toast ? <p style={{ color: "var(--ok)", fontSize: 13, marginBottom: 8 }}>{toast}</p> : null}

      <div
        style={{
          position: "relative",
          flex: 1,
          minHeight: 320,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        {knocks.length === 0 && (
          <p style={{ color: "var(--ink-faint)" }}>Nothing at the door.</p>
        )}
        <AnimatePresence>
          {top3
            .slice()
            .reverse()
            .map((k, iFromBack) => {
              const isTop = iFromBack === top3.length - 1;
              const depth = top3.length - 1 - iFromBack;
              return (
                <SwipeCard
                  key={k.id}
                  knock={k}
                  isTop={isTop}
                  depth={depth}
                  busy={busy}
                  onApprove={() => {
                    setTsToken(null);
                    setConfirming(k);
                  }}
                  onRefuse={() => doRefuse(k.id)}
                />
              );
            })}
        </AnimatePresence>
      </div>

      <AnimatePresence>
        {confirming ? (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            style={sheetBackdrop}
            onClick={() => {
              setConfirming(null);
              setTsToken(null);
            }}
          >
            <motion.div
              initial={{ y: 40, opacity: 0 }}
              animate={{ y: 0, opacity: 1 }}
              exit={{ y: 40, opacity: 0 }}
              style={sheetCard}
              onClick={(e) => e.stopPropagation()}
            >
              <h3 style={{ margin: "0 0 8px" }}>{confirming.name} is at the door</h3>
              {confirming.permissions.map((p, i) => (
                <p key={i} style={{ color: "#ccc", margin: "2px 0" }}>
                  · {p} {confirming.resources[i] ?? ""}
                </p>
              ))}
              <p style={{ color: "var(--ink-dim)", fontSize: 13, margin: "12px 0" }}>
                Reason: {confirming.reason}
              </p>
              <p style={{ color: "var(--ink-dim)", fontSize: 13, marginBottom: 12 }}>
                Do you recognize this name?
              </p>
              {tsToken === null ? (
                <TurnstileGate onToken={setTsToken} />
              ) : (
                <p style={{ color: "var(--ok)", padding: 12 }}>Human verified.</p>
              )}
              <div style={{ display: "flex", gap: 8, marginTop: 12 }}>
                <button
                  style={ghostBtn}
                  onClick={() => {
                    setConfirming(null);
                    setTsToken(null);
                  }}
                >
                  CANCEL
                </button>
                <button
                  style={{ ...solidBtn, opacity: tsToken && !busy ? 1 : 0.5 }}
                  disabled={!tsToken || busy}
                  onClick={doApprove}
                >
                  CONFIRM
                </button>
              </div>
            </motion.div>
          </motion.div>
        ) : null}
      </AnimatePresence>
    </div>
  );
}

function SwipeCard({
  knock,
  isTop,
  depth,
  busy,
  onApprove,
  onRefuse,
}: {
  knock: api.Knock;
  isTop: boolean;
  depth: number;
  busy: boolean;
  onApprove: () => void;
  onRefuse: () => void;
}) {
  const x = useMotionValue(0);
  const rotate = useTransform(x, [-300, 300], [-18, 18]);
  const approveOpacity = useTransform(x, [20, 140], [0, 1]);
  const refuseOpacity = useTransform(x, [-140, -20], [1, 0]);

  return (
    <motion.div
      drag={isTop ? "x" : false}
      style={{
        x: isTop ? x : 0,
        rotate: isTop ? rotate : 0,
        position: "absolute",
        width: "min(360px, 88vw)",
        zIndex: 10 + depth,
      }}
      initial={{ scale: 0.94, y: 16, opacity: 0 }}
      animate={{
        scale: 1 - (2 - Math.min(depth, 2)) * 0.04,
        y: (2 - Math.min(depth, 2)) * -10,
        opacity: 1,
      }}
      exit={{ x: x.get() > 0 ? 400 : -400, opacity: 0, transition: { duration: 0.25 } }}
      dragConstraints={{ left: 0, right: 0 }}
      dragElastic={1}
      onDragEnd={(_, info) => {
        if (info.offset.x > SWIPE_THRESHOLD || info.velocity.x > SWIPE_VELOCITY) {
          onApprove();
        } else if (info.offset.x < -SWIPE_THRESHOLD || info.velocity.x < -SWIPE_VELOCITY) {
          onRefuse();
        }
      }}
    >
      <div
        style={{
          background: "var(--card)",
          border: `1px solid ${knock.seconds_remaining < 20 ? "#a44" : "var(--line)"}`,
          borderRadius: 18,
          padding: 20,
          boxShadow: "0 12px 40px rgba(0,0,0,0.45)",
          position: "relative",
          overflow: "hidden",
        }}
      >
        {isTop && (
          <>
            <motion.span style={{ ...badgeTag, ...badgeApprove, opacity: approveOpacity }}>
              APPROVE
            </motion.span>
            <motion.span style={{ ...badgeTag, ...badgeRefuse, opacity: refuseOpacity }}>
              REFUSE
            </motion.span>
          </>
        )}
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <h3 style={{ margin: 0, fontSize: 18 }}>{knock.name}</h3>
          <span
            style={{
              fontSize: 12,
              fontWeight: 700,
              color: knock.seconds_remaining < 20 ? "var(--urgent)" : "var(--ok)",
            }}
          >
            {Math.max(0, Math.floor(knock.seconds_remaining))}s
          </span>
        </div>
        <div style={{ margin: "10px 0" }}>
          {knock.permissions.map((p, i) => (
            <p key={i} style={{ color: "#ccc", margin: "2px 0", fontSize: 14 }}>
              · {p} {knock.resources[i] ?? ""}
            </p>
          ))}
        </div>
        <p style={{ color: "var(--ink-dim)", fontSize: 13 }}>Reason: {knock.reason}</p>
        <div style={{ display: "flex", gap: 8, marginTop: 16 }}>
          <button style={ghostBtn} disabled={busy} onClick={onRefuse}>
            REFUSE
          </button>
          <button style={solidBtn} disabled={busy} onClick={onApprove}>
            APPROVE
          </button>
        </div>
      </div>
    </motion.div>
  );
}

const solidBtn: React.CSSProperties = {
  flex: 1,
  background: "var(--accent)",
  color: "#000",
  fontWeight: 700,
  padding: 12,
  borderRadius: 10,
  border: "none",
  cursor: "pointer",
};

const ghostBtn: React.CSSProperties = {
  flex: 1,
  background: "transparent",
  color: "#ccc",
  fontWeight: 700,
  padding: 12,
  borderRadius: 10,
  border: "1px solid #444",
  cursor: "pointer",
};

const badgeTag: React.CSSProperties = {
  position: "absolute",
  top: 16,
  fontSize: 12,
  fontWeight: 800,
  letterSpacing: 1,
  padding: "4px 10px",
  borderRadius: 6,
  border: "2px solid",
};

const badgeApprove: React.CSSProperties = {
  right: 16,
  color: "var(--ok)",
  borderColor: "var(--ok)",
  transform: "rotate(-8deg)",
};

const badgeRefuse: React.CSSProperties = {
  left: 16,
  color: "var(--urgent)",
  borderColor: "var(--urgent)",
  transform: "rotate(8deg)",
};

const sheetBackdrop: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  background: "rgba(0,0,0,0.75)",
  display: "flex",
  alignItems: "flex-end",
  justifyContent: "center",
  zIndex: 100,
};

const sheetCard: React.CSSProperties = {
  background: "var(--panel)",
  borderRadius: "18px 18px 0 0",
  padding: 24,
  width: "100%",
  maxWidth: 480,
};
