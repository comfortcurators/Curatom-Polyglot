import React, { useCallback, useEffect, useState } from "react";
import {
  SafeAreaView, ScrollView, View, Text, Pressable, StyleSheet,
  RefreshControl, ActivityIndicator, Modal,
} from "react-native";
import { StatusBar } from "expo-status-bar";
import * as Clipboard from "expo-clipboard";
import * as api from "./lib/api";
import EnrollmentScreen from "./screens/EnrollmentScreen";
import TurnstileGate from "./components/TurnstileGate";

type Tab = "token" | "knocks" | "activity";

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
        {(["token", "knocks", "activity"] as Tab[]).map((t) => (
          <Pressable key={t} onPress={() => setTab(t)} style={[s.tab, tab === t && s.tabActive]}>
            <Text style={[s.tabText, tab === t && s.tabTextActive]}>{t.toUpperCase()}</Text>
          </Pressable>
        ))}
      </View>
      {tab === "token" && <TokenTab />}
      {tab === "knocks" && <KnocksTab />}
      {tab === "activity" && <ActivityTab />}
    </SafeAreaView>
  );
}

function TokenTab() {
  const [info, setInfo] = useState<api.TokenInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try { setInfo(await api.getToken()); setError(null); }
    catch (e) { setError(String(e)); }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  async function copy() {
    if (!info) return;
    await Clipboard.setStringAsync(info.file_text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }

  async function rotate() {
    setBusy(true); setError(null);
    try { await api.rotateToken(); await refresh(); }
    catch (e) { setError(String(e)); }
    setBusy(false);
  }

  if (!info) return <View style={s.center}><ActivityIndicator color="#fff" /></View>;

  return (
    <ScrollView contentContainerStyle={s.pad}>
      <Text style={s.h1}>Your token</Text>
      <Text style={s.dim}>
        Copy this. Give it to any AI. They will read it and come to Curatom.
        You will see their knock here.
      </Text>

      <View style={s.codeBox}>
        <Text style={s.code}>{info.file_text}</Text>
      </View>

      <View style={s.row}>
        <Pressable onPress={copy} style={s.btn}>
          <Text style={s.btnText}>{copied ? "COPIED" : "COPY"}</Text>
        </Pressable>
        <Pressable onPress={rotate} style={[s.btn, s.btnGhost]} disabled={busy}>
          {busy ? <ActivityIndicator color="#ccc" /> :
            <Text style={s.btnGhostText}>REROLL</Text>}
        </Pressable>
      </View>

      {error ? <Text style={s.err}>{error}</Text> : null}
    </ScrollView>
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
          <Text style={s.dim}>came with your token</Text>
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
  codeBox: { backgroundColor: "#101012", borderRadius: 12, padding: 14, borderWidth: 1, borderColor: "#222" },
  code: { color: "#bbb", fontFamily: "monospace", fontSize: 12, lineHeight: 18 },
  btn: { backgroundColor: "#eee", padding: 12, borderRadius: 10, alignItems: "center", flex: 1 },
  btnDim: { opacity: 0.5 },
  btnGhost: { backgroundColor: "transparent", borderWidth: 1, borderColor: "#444" },
  btnText: { color: "#000", fontWeight: "700" },
  btnGhostText: { color: "#ccc", fontWeight: "700" },
  card: { borderWidth: 1, borderColor: "#2a2a2a", borderRadius: 12, padding: 14, gap: 6 },
  cardUrgent: { borderColor: "#a44" },
  cardTitle: { color: "#fff", fontWeight: "700", fontSize: 16 },
  err: { color: "#f66", fontSize: 16, fontWeight: "700" },
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
});
