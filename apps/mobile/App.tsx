import React, { useCallback, useEffect, useState } from "react";
import {
  SafeAreaView, ScrollView, View, Text, Pressable, StyleSheet,
  RefreshControl, ActivityIndicator, Modal, TextInput,
} from "react-native";
import { StatusBar } from "expo-status-bar";
import * as Clipboard from "expo-clipboard";
import * as api from "./lib/api";
import EnrollmentScreen from "./screens/EnrollmentScreen";
import TurnstileGate from "./components/TurnstileGate";

type Tab = "keys" | "knocks" | "billboard" | "activity";

export default function App() {
  const [me, setMe] = useState<api.Me | null>(null);
  const [bootError, setBootError] = useState<string | null>(null);

  const refreshMe = useCallback(async () => {
    try { setMe(await api.me()); setBootError(null); }
    catch (e) { setBootError(String(e)); }
  }, []);

  useEffect(() => { refreshMe(); }, [refreshMe]);

  if (bootError) return (
    <SafeAreaView style={s.root}><View style={s.center}>
      <Text style={s.err}>Could not reach Curatom.</Text>
      <Text style={s.dim}>{bootError}</Text>
      <Pressable onPress={refreshMe} style={s.btn}><Text style={s.btnText}>RETRY</Text></Pressable>
    </View></SafeAreaView>
  );

  if (!me) return (
    <SafeAreaView style={s.root}><View style={s.center}>
      <ActivityIndicator color="#fff" />
    </View></SafeAreaView>
  );

  if (!me.enrolled) return (
    <SafeAreaView style={s.root}><StatusBar style="light" />
      <EnrollmentScreen onEnrolled={refreshMe} />
    </SafeAreaView>
  );

  return <Dashboard />;
}

function Dashboard() {
  const [tab, setTab] = useState<Tab>("knocks");
  return (
    <SafeAreaView style={s.root}>
      <StatusBar style="light" />
      <View style={s.tabBar}>
        {(["keys", "knocks", "billboard", "activity"] as Tab[]).map((t) => (
          <Pressable key={t} onPress={() => setTab(t)} style={[s.tab, tab === t && s.tabActive]}>
            <Text style={[s.tabText, tab === t && s.tabTextActive]}>{t.toUpperCase()}</Text>
          </Pressable>
        ))}
      </View>
      {tab === "keys" && <KeysTab />}
      {tab === "knocks" && <KnocksTab />}
      {tab === "billboard" && <BillboardTab />}
      {tab === "activity" && <ActivityTab />}
    </SafeAreaView>
  );
}

function KeysTab() {
  const [keys, setKeys] = useState<api.KeyInfo[]>([]);
  const [creating, setCreating] = useState(false);
  const [detail, setDetail] = useState<api.KeyInfo | null>(null);
  const [lastFile, setLastFile] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try { setKeys(await api.listKeys()); setError(null); }
    catch (e) { setError(String(e)); }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  async function revoke(token: string) {
    setBusy(true);
    try { await api.revokeKey(token); await refresh(); }
    catch (e) { setError(String(e)); }
    setBusy(false);
  }

  return (
    <ScrollView contentContainerStyle={s.pad}
      refreshControl={<RefreshControl refreshing={false} onRefresh={refresh} />}>
      <Pressable onPress={() => setCreating(true)} style={s.btn}>
        <Text style={s.btnText}>+ NEW KEY</Text>
      </Pressable>
      {error ? <Text style={s.err}>{error}</Text> : null}
      {keys.length === 0 && <Text style={s.dim}>No keys yet. Create one to hand to an AI.</Text>}

      {keys.map((k) => (
        <View key={k.token} style={s.card}>
          <View style={s.rowTop}>
            <Text style={s.cardTitle}>{k.label}</Text>
            <Text style={s.dim}>{k.activity_count} activities</Text>
          </View>
          <Text style={s.mono}>{k.token.slice(0, 24)}…</Text>
          <Text style={s.dim}>created {k.created_at}</Text>
          {k.last_used_at ? <Text style={s.dim}>last used {k.last_used_at}</Text> : null}
          <View style={s.row}>
            <Pressable onPress={() => setDetail(k)} style={[s.btn, s.btnGhost]} disabled={busy}>
              <Text style={s.btnGhostText}>LOG</Text>
            </Pressable>
            <Pressable onPress={() => revoke(k.token)} style={[s.btn, s.btnGhost]} disabled={busy}>
              <Text style={s.btnGhostText}>REVOKE</Text>
            </Pressable>
          </View>
        </View>
      ))}

      <CreateKeyModal
        visible={creating}
        onClose={() => setCreating(false)}
        onCreated={(file_text) => { setLastFile(file_text); setCreating(false); refresh(); }}
      />

      <Modal visible={lastFile !== null} transparent animationType="fade"
        onRequestClose={() => setLastFile(null)}>
        <View style={s.modalBackdrop}>
          <ScrollView contentContainerStyle={s.modalCard}>
            <Text style={s.modalH}>Give this to an AI</Text>
            <View style={s.codeBox}>
              <Text style={s.code}>{lastFile}</Text>
            </View>
            <Pressable
              onPress={async () => {
                if (lastFile) { await Clipboard.setStringAsync(lastFile); }
                setLastFile(null);
              }}
              style={s.btn}>
              <Text style={s.btnText}>COPY & CLOSE</Text>
            </Pressable>
          </ScrollView>
        </View>
      </Modal>

      <KeyLogModal info={detail} onClose={() => setDetail(null)} />
    </ScrollView>
  );
}

function CreateKeyModal({ visible, onClose, onCreated }: {
  visible: boolean; onClose: () => void; onCreated: (file_text: string) => void;
}) {
  const [label, setLabel] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit() {
    setBusy(true); setError(null);
    try {
      const r = await api.createKey(label.trim());
      setLabel("");
      onCreated(r.file_text);
    } catch (e) { setError(String(e)); }
    setBusy(false);
  }

  return (
    <Modal visible={visible} transparent animationType="slide" onRequestClose={onClose}>
      <View style={s.modalBackdrop}>
        <View style={s.modalCard}>
          <Text style={s.modalH}>New key</Text>
          <Text style={s.modalDim}>Name it however you like. This label is yours. The AI never sees it.</Text>
          <TextInput value={label} onChangeText={setLabel}
            placeholder="e.g. Claude laptop, work key"
            placeholderTextColor="#666" style={s.input} />
          {error ? <Text style={s.err}>{error}</Text> : null}
          <View style={s.row}>
            <Pressable onPress={onClose} style={[s.btn, s.btnGhost]} disabled={busy}>
              <Text style={s.btnGhostText}>CANCEL</Text>
            </Pressable>
            <Pressable onPress={submit} style={[s.btn, busy && s.btnDim]} disabled={busy || !label.trim()}>
              {busy ? <ActivityIndicator color="#000" /> : <Text style={s.btnText}>CREATE</Text>}
            </Pressable>
          </View>
        </View>
      </View>
    </Modal>
  );
}

function KeyLogModal({ info, onClose }: { info: api.KeyInfo | null; onClose: () => void }) {
  const [log, setLog] = useState<api.KeyLogEntry[]>([]);
  useEffect(() => {
    if (!info) { setLog([]); return; }
    api.keyLog(info.token).then(setLog).catch(() => setLog([]));
  }, [info]);
  return (
    <Modal visible={info !== null} transparent animationType="fade" onRequestClose={onClose}>
      <View style={s.modalBackdrop}>
        <ScrollView contentContainerStyle={s.modalCard}>
          <Text style={s.modalH}>{info?.label}</Text>
          <Text style={s.modalDim}>{log.length} entries</Text>
          {log.map((e, i) => (
            <View key={i} style={s.logRow}>
              <Text style={s.logKind}>{e.kind}</Text>
              <Text style={s.dim}>{e.at}</Text>
              {e.name ? <Text style={s.dim}>by {e.name}</Text> : null}
              {e.reason ? <Text style={s.dim}>{e.reason}</Text> : null}
            </View>
          ))}
          {log.length === 0 && <Text style={s.dim}>No activity yet.</Text>}
          <Pressable onPress={onClose} style={s.btn}>
            <Text style={s.btnText}>CLOSE</Text>
          </Pressable>
        </ScrollView>
      </View>
    </Modal>
  );
}

function KnocksTab() {
  const [knocks, setKnocks] = useState<api.Knock[]>([]);
  const [busy, setBusy] = useState(false);
  const [last, setLast] = useState("");
  const [confirming, setConfirming] = useState<api.Knock | null>(null);
  const [tsToken, setTsToken] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try { setKnocks(await api.listKnocks()); } catch (e) { setLast(String(e)); }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);
  useEffect(() => {
    const iv = setInterval(refresh, 2000);
    return () => clearInterval(iv);
  }, [refresh]);

  async function doRefuse(id: string) {
    setBusy(true);
    try { await api.refuseKnock(id); setLast("Refused."); await refresh(); }
    catch (e) { setLast(String(e)); }
    setBusy(false);
  }

  async function doApprove() {
    if (!confirming || !tsToken) return;
    const id = confirming.id, token = tsToken;
    setConfirming(null); setTsToken(null); setBusy(true);
    try { await api.approveKnock(id, token); setLast("Approved."); await refresh(); }
    catch (e) { setLast(String(e)); }
    setBusy(false);
  }

  return (
    <ScrollView contentContainerStyle={s.pad}
      refreshControl={<RefreshControl refreshing={false} onRefresh={refresh} />}>
      {last ? <Text style={s.result}>{last}</Text> : null}
      {knocks.length === 0 && <Text style={s.dim}>Nothing at the door.</Text>}
      {knocks.map((k) => (
        <View key={k.id} style={[s.card, k.seconds_remaining < 20 && s.cardUrgent]}>
          <View style={s.rowTop}>
            <Text style={s.cardTitle}>{k.name}</Text>
            <Text style={[s.badge, k.seconds_remaining < 20 ? s.badgeUrgent : s.badgeOk]}>
              {Math.max(0, Math.floor(k.seconds_remaining))}s
            </Text>
          </View>
          {k.permissions.map((p, i) => (
            <Text key={i} style={s.perm}>· {p} {k.resources[i] ?? ""}</Text>
          ))}
          <Text style={s.dim}>Reason: {k.reason}</Text>
          <View style={s.row}>
            <Pressable onPress={() => doRefuse(k.id)} style={[s.btn, s.btnGhost]} disabled={busy}>
              <Text style={s.btnGhostText}>REFUSE</Text>
            </Pressable>
            <Pressable onPress={() => { setTsToken(null); setConfirming(k); }}
              style={s.btn} disabled={busy}>
              <Text style={s.btnText}>APPROVE</Text>
            </Pressable>
          </View>
        </View>
      ))}

      <Modal visible={confirming !== null} transparent animationType="fade"
        onRequestClose={() => { setConfirming(null); setTsToken(null); }}>
        <View style={s.modalBackdrop}>
          <View style={s.modalCard}>
            <Text style={s.modalH}>{confirming?.name} is at the door</Text>
            {confirming ? (
              <>
                {confirming.permissions.map((p, i) => (
                  <Text key={i} style={s.perm}>· {p} {confirming.resources[i] ?? ""}</Text>
                ))}
                <Text style={s.modalDim}>Reason: {confirming.reason}</Text>
                <Text style={s.modalDim}>Do you recognize this name?</Text>
              </>
            ) : null}
            {tsToken === null
              ? <TurnstileGate onToken={setTsToken} />
              : <Text style={s.modalOk}>Human verified.</Text>}
            <View style={s.row}>
              <Pressable onPress={() => { setConfirming(null); setTsToken(null); }}
                style={[s.btn, s.btnGhost]}>
                <Text style={s.btnGhostText}>CANCEL</Text>
              </Pressable>
              <Pressable onPress={doApprove}
                style={[s.btn, (!tsToken || busy) && s.btnDim]}
                disabled={!tsToken || busy}>
                <Text style={s.btnText}>CONFIRM</Text>
              </Pressable>
            </View>
          </View>
        </View>
      </Modal>
    </ScrollView>
  );
}

function BillboardTab() {
  const [entries, setEntries] = useState<api.BillboardEntry[]>([]);
  const [filter, setFilter] = useState<"all" | "intent" | "pattern">("all");
  const [busy, setBusy] = useState(false);
  const [detail, setDetail] = useState<{ ref: string; body: string } | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setBusy(true);
    try {
      const list = await api.billboard({
        kind: filter === "all" ? undefined : filter,
        limit: 200,
      });
      setEntries(list);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
    setBusy(false);
  }, [filter]);

  useEffect(() => { refresh(); }, [refresh]);

  async function openBody(ref: string) {
    try {
      const body = await api.billboardBlob(ref);
      setDetail({ ref, body });
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <ScrollView contentContainerStyle={s.pad}
      refreshControl={<RefreshControl refreshing={busy} onRefresh={refresh} />}>
      <View style={s.row}>
        {(["all", "intent", "pattern"] as const).map((f) => (
          <Pressable key={f} onPress={() => setFilter(f)}
            style={[s.chip, filter === f && s.chipOn]}>
            <Text style={[s.chipText, filter === f && s.chipTextOn]}>{f.toUpperCase()}</Text>
          </Pressable>
        ))}
      </View>

      {error ? <Text style={s.err}>{error}</Text> : null}
      {entries.length === 0 && !busy && <Text style={s.dim}>No entries yet.</Text>}

      {entries.map((e) => (
        <Pressable key={e.id} onPress={() => openBody(e.body_ref)} style={s.card}>
          <View style={s.rowTop}>
            <Text style={s.cardTitle}>{e.kind.toUpperCase()} · {e.key_label || e.key_hash.slice(0, 10)}</Text>
            <Text style={s.dim}>r{e.round}</Text>
          </View>
          <Text style={s.dim}>{e.created_at}</Text>
          {e.knock_id ? <Text style={s.dim}>knock {e.knock_id.slice(0, 14)}…</Text> : null}
        </Pressable>
      ))}

      <Modal visible={detail !== null} transparent animationType="fade"
        onRequestClose={() => setDetail(null)}>
        <View style={s.modalBackdrop}>
          <ScrollView contentContainerStyle={s.modalCard}>
            <Text style={s.modalH}>Body</Text>
            <Text style={s.dim}>{detail?.ref}</Text>
            <View style={s.codeBox}>
              <Text style={s.code}>{detail?.body}</Text>
            </View>
            <Pressable onPress={() => setDetail(null)} style={s.btn}>
              <Text style={s.btnText}>CLOSE</Text>
            </Pressable>
          </ScrollView>
        </View>
      </Modal>
    </ScrollView>
  );
}

function ActivityTab() {
  const [events, setEvents] = useState<{ kind: string; at: string; actor: string }[]>([]);
  const refresh = useCallback(async () => {
    try { setEvents(await api.activity()); } catch {}
  }, []);
  useEffect(() => { refresh(); }, [refresh]);
  return (
    <ScrollView contentContainerStyle={s.pad}
      refreshControl={<RefreshControl refreshing={false} onRefresh={refresh} />}>
      {events.slice().reverse().map((e, i) => (
        <Text key={i} style={s.dim}>{e.kind}</Text>
      ))}
    </ScrollView>
  );
}

const s = StyleSheet.create({
  root: { flex: 1, backgroundColor: "#0b0b0c" },
  center: { flex: 1, alignItems: "center", justifyContent: "center", gap: 12, padding: 24 },
  pad: { padding: 20, gap: 12 },
  h1: { color: "#fff", fontSize: 24, fontWeight: "700" },
  dim: { color: "#999" },
  perm: { color: "#ccc" },
  mono: { color: "#bbb", fontFamily: "monospace", fontSize: 13 },
  codeBox: { backgroundColor: "#101012", borderRadius: 12, padding: 14, borderWidth: 1, borderColor: "#222", marginVertical: 12 },
  code: { color: "#bbb", fontFamily: "monospace", fontSize: 12, lineHeight: 18 },
  btn: { backgroundColor: "#eee", padding: 12, borderRadius: 10, alignItems: "center", flex: 1 },
  btnDim: { opacity: 0.5 },
  btnGhost: { backgroundColor: "transparent", borderWidth: 1, borderColor: "#444" },
  btnText: { color: "#000", fontWeight: "700" },
  btnGhostText: { color: "#ccc", fontWeight: "700" },
  card: { borderWidth: 1, borderColor: "#2a2a2a", borderRadius: 12, padding: 14, gap: 6 },
  cardUrgent: { borderColor: "#a44" },
  cardTitle: { color: "#fff", fontWeight: "700", fontSize: 16 },
  err: { color: "#f66", fontSize: 14, fontWeight: "700" },
  row: { flexDirection: "row", gap: 8, marginTop: 8 },
  rowTop: { flexDirection: "row", justifyContent: "space-between", alignItems: "center" },
  result: { color: "#8f8", marginTop: 8 },
  tabBar: { flexDirection: "row", borderBottomWidth: 1, borderBottomColor: "#222" },
  tab: { flex: 1, paddingVertical: 14, alignItems: "center" },
  tabActive: { borderBottomWidth: 2, borderBottomColor: "#fff" },
  tabText: { color: "#666", fontWeight: "700", fontSize: 12, letterSpacing: 1 },
  tabTextActive: { color: "#fff" },
  badge: { fontSize: 12, fontWeight: "700" },
  badgeOk: { color: "#8f8" },
  badgeUrgent: { color: "#f66" },
  modalBackdrop: { flex: 1, backgroundColor: "rgba(0,0,0,0.75)", justifyContent: "center", padding: 20 },
  modalCard: { backgroundColor: "#141416", borderRadius: 14, padding: 20, gap: 12 },
  modalH: { color: "#fff", fontSize: 18, fontWeight: "700" },
  modalDim: { color: "#888", fontSize: 13, lineHeight: 20 },
  modalOk: { color: "#8f8", fontSize: 14, padding: 12 },
  input: { color: "#fff", borderWidth: 1, borderColor: "#333", borderRadius: 10, padding: 12, marginTop: 8 },
  logRow: { borderTopWidth: 1, borderTopColor: "#222", paddingVertical: 8, gap: 2 },
  logKind: { color: "#fff", fontWeight: "600" },
  chip: { paddingHorizontal: 12, paddingVertical: 6, borderRadius: 6, borderWidth: 1, borderColor: "#333" },
  chipOn: { backgroundColor: "#eee", borderColor: "#eee" },
  chipText: { color: "#666", fontSize: 12, fontWeight: "600" },
  chipTextOn: { color: "#000" },
});
