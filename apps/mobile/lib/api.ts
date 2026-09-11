const BASE = process.env.EXPO_PUBLIC_CURATOM_API ?? "http://localhost:8787";

// In production this is Cloudflare Access. For local dev, the Worker's
// DevOrganicIdentityProvider reads this header.
function devHeaders(): Record<string, string> {
  const id = process.env.EXPO_PUBLIC_DEV_ORGANIC_ID;
  return id ? { "x-curatom-dev-organic": id } : {};
}

export async function me() {
  const r = await fetch(`${BASE}/organic/me`, { headers: devHeaders() });
  if (!r.ok) throw new Error(`me: ${r.status}`);
  return r.json();
}

export async function createIntent(text: string) {
  const r = await fetch(`${BASE}/organic/intents`, {
    method: "POST",
    headers: { "content-type": "application/json", ...devHeaders() },
    body: JSON.stringify({ text }),
  });
  if (!r.ok) throw new Error(`intent: ${r.status}`);
  return r.json();
}

export async function listApprovals() {
  const r = await fetch(`${BASE}/organic/approvals`, { headers: devHeaders() });
  if (!r.ok) throw new Error(`approvals: ${r.status}`);
  return r.json();
}

export async function approve(approvalId: string, sign2: string) {
  const r = await fetch(`${BASE}/organic/approvals/${approvalId}/approve`, {
    method: "POST",
    headers: { "content-type": "application/json", ...devHeaders() },
    body: JSON.stringify({ sign2 }),
  });
  if (!r.ok) throw new Error(`approve: ${r.status}`);
  return r.json();
}

export async function refuse(approvalId: string) {
  const r = await fetch(`${BASE}/organic/approvals/${approvalId}/refuse`, {
    method: "POST",
    headers: { ...devHeaders() },
  });
  if (!r.ok) throw new Error(`refuse: ${r.status}`);
  return r.json();
}

export async function activity() {
  const r = await fetch(`${BASE}/organic/activity`, { headers: devHeaders() });
  if (!r.ok) throw new Error(`activity: ${r.status}`);
  return r.json();
}
