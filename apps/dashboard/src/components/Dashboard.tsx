import { useCallback, useEffect, useState } from "react";
import { AnimatePresence, LayoutGroup, motion } from "framer-motion";
import * as api from "@/lib/api";
import { PLATES, type PlateId } from "@/lib/plates";
import { useCounts } from "@/lib/useCounts";
import KnocksPlate from "@/components/KnocksPlate";
import KeysPlate from "@/components/KeysPlate";
import ValhallaPlate from "@/components/ValhallaPlate";
import BillboardPlate from "@/components/BillboardPlate";
import ActivityPlate from "@/components/ActivityPlate";

const PLATE_VIEW: Record<PlateId, () => JSX.Element> = {
  knocks: KnocksPlate,
  keys: KeysPlate,
  valhalla: ValhallaPlate,
  billboard: BillboardPlate,
  activity: ActivityPlate,
};

export default function Dashboard() {
  const [zoomed, setZoomed] = useState<PlateId | null>(null);
  const [frozen, setFrozen] = useState<api.Frozen[]>([]);
  const counts = useCounts();

  useEffect(() => {
    const refresh = () => api.frozenList().then(setFrozen).catch(() => {});
    refresh();
    const iv = setInterval(refresh, 5000);
    return () => clearInterval(iv);
  }, []);

  const close = useCallback(() => setZoomed(null), []);

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      {frozen.length > 0 && (
        <div style={{ background: "#a44", padding: "10px 16px", flexShrink: 0 }}>
          <p style={{ color: "#fff", fontWeight: 700, fontSize: 13, margin: 0 }}>
            {frozen.length} resource{frozen.length === 1 ? "" : "s"} frozen by Valhalla
          </p>
          <p style={{ color: "#fee", fontSize: 11, margin: "2px 0 0" }}>
            {frozen.map((f) => f.scope).join(", ")}
          </p>
        </div>
      )}

      <LayoutGroup>
        <div style={{ flex: 1, position: "relative", overflow: "hidden" }}>
          <AnimatePresence>
            {zoomed === null && (
              <motion.div
                key="overview"
                exit={{ opacity: 0 }}
                style={{ position: "absolute", inset: 0, overflowY: "auto" }}
              >
                <Overview counts={counts} onOpen={setZoomed} />
              </motion.div>
            )}
          </AnimatePresence>

          <AnimatePresence>
            {zoomed !== null && (
              <PlateFullscreen key={zoomed} plate={zoomed} onClose={close} />
            )}
          </AnimatePresence>
        </div>
      </LayoutGroup>
    </div>
  );
}

function Overview({
  counts,
  onOpen,
}: {
  counts: Partial<Record<PlateId, number>>;
  onOpen: (p: PlateId) => void;
}) {
  return (
    <div style={{ padding: 24 }}>
      <div style={{ marginBottom: 20 }}>
        <p style={{ color: "var(--gold)", letterSpacing: 2, fontSize: 11, fontWeight: 700, margin: 0 }}>
          CURATOM
        </p>
        <h1 style={{ margin: "4px 0 0", fontSize: 26, fontWeight: 700 }}>The whole door.</h1>
      </div>
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(150px, 1fr))",
          gap: 14,
        }}
      >
        {PLATES.map((p) => (
          <motion.button
            key={p.id}
            layoutId={`plate-${p.id}`}
            onClick={() => onOpen(p.id)}
            whileTap={{ scale: 0.97 }}
            style={{
              textAlign: "left",
              border: "1px solid var(--line)",
              borderRadius: 18,
              background: "var(--card)",
              padding: 18,
              minHeight: 132,
              cursor: "pointer",
              display: "flex",
              flexDirection: "column",
              justifyContent: "space-between",
              color: "inherit",
              font: "inherit",
            }}
          >
            <motion.span layoutId={`plate-${p.id}-title`} style={{ fontSize: 16, fontWeight: 700 }}>
              {p.title}
            </motion.span>
            <span>
              <span style={{ fontSize: 32, fontWeight: 800, lineHeight: 1 }}>
                {counts[p.id] ?? "–"}
              </span>
              <p style={{ color: "var(--ink-faint)", fontSize: 12, margin: "6px 0 0" }}>{p.blurb}</p>
            </span>
          </motion.button>
        ))}
      </div>
    </div>
  );
}

function PlateFullscreen({ plate, onClose }: { plate: PlateId; onClose: () => void }) {
  const View = PLATE_VIEW[plate];
  const meta = PLATES.find((p) => p.id === plate)!;

  return (
    <motion.div
      layoutId={`plate-${plate}`}
      drag="y"
      dragConstraints={{ top: 0, bottom: 0 }}
      dragElastic={0.4}
      onDragEnd={(_, info) => {
        if (info.offset.y > 100 || info.velocity.y > 500) onClose();
      }}
      style={{
        position: "absolute",
        inset: 0,
        background: "var(--bg)",
        display: "flex",
        flexDirection: "column",
        borderRadius: 0,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 10,
          padding: "16px 24px 0",
          flexShrink: 0,
        }}
      >
        <button
          onClick={onClose}
          style={{
            background: "transparent",
            border: "1px solid #333",
            color: "#ccc",
            borderRadius: 8,
            width: 32,
            height: 32,
            cursor: "pointer",
            fontSize: 16,
          }}
          aria-label="Back to the whole map"
        >
          ←
        </button>
        <motion.span layoutId={`plate-${plate}-title`} style={{ fontSize: 13, color: "var(--ink-faint)" }}>
          {meta.title}
        </motion.span>
      </div>
      <div style={{ flex: 1, minHeight: 0 }}>
        <View />
      </div>
    </motion.div>
  );
}
