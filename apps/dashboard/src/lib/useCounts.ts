import { useEffect, useState } from "react";
import * as api from "@/lib/api";
import type { PlateId } from "@/lib/plates";

export function useCounts() {
  const [counts, setCounts] = useState<Partial<Record<PlateId, number>>>({});

  useEffect(() => {
    let cancelled = false;

    async function refresh() {
      const [knocks, keys, valhalla, billboard, activity] = await Promise.all([
        api.listKnocks().catch(() => []),
        api.listKeys().catch(() => []),
        api.valhallaSessions().catch(() => []),
        api.billboard({ limit: 200 }).catch(() => []),
        api.activity().catch(() => []),
      ]);
      if (cancelled) return;
      setCounts({
        knocks: knocks.length,
        keys: keys.length,
        valhalla: valhalla.filter((v) => v.alive).length,
        billboard: billboard.length,
        activity: activity.length,
      });
    }

    refresh();
    const iv = setInterval(refresh, 3000);
    return () => {
      cancelled = true;
      clearInterval(iv);
    };
  }, []);

  return counts;
}
