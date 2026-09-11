import React, { useCallback, useEffect, useState } from "react";
import { SafeAreaView, ScrollView, View, Text, TextInput, Pressable, StyleSheet, RefreshControl, ActivityIndicator } from "react-native";
import { StatusBar } from "expo-status-bar";
import * as api from "./lib/api";
import EnrollmentScreen from "./screens/EnrollmentScreen";
import SignatureCapture from "./screens/SignatureCapture";

type ApprovalView = {
  id: string;
  requester: string;
  reason: string;
  resources: string[];
  permissions: string[];
  duration: string;
};

export default function App() {
  const [me, setMe] = useState<api.Me | null>(null);
  const [bootError, setBootError] = useState<string | null>(null);

  async function refreshMe() {
    try {
      const m = await api.me();
      setMe(m);
      setBootError(null);
    } catch (e) {
      setBootError(String(e));
    }
  }

  useEffect(() => { refreshMe(); }, []);

  if (bootError) {
    return (
      <SafeAreaView style={s.root}>
        <View style={s.center}>
          <Text style={s.err}>Could not reach Curatom.</Text>
          <Text style={s.dim}>{bootError}</Text>
          <Pressable onPress={refreshMe} style={s.btn}><Text style={s.btnText}>RETRY</Text></Pressable>
        </View>
      </SafeAreaView>
    );
  }

  if (!me) {
    return (
      <SafeAreaView style={s.root}>
        <View style={s.center}><ActivityIndicator color="#fff" /></View>
      </SafeAreaView>
    );
  }

  if (!me.enrolled) {
    return (
      <SafeAreaView style={s.root}>
        <StatusBar style="light" />
        <EnrollmentScreen onEnrolled={refreshMe} />
      </SafeAreaView>
    );
  }

  return <Main onRefreshMe={refreshMe} />;
}

function Main({ onRefreshMe }: { onRefreshMe: () => void }) {
  const [intent, setIntent] = useState("");
  const [approvals, setApprovals] = useState<ApprovalView[]>([]);
  const [last, setLast] = useState("");
  const [activity, setActivity] = useState<{ kind: string }[]>([]);
  const [busy, setBusy] = useState(false);
  const [signing, setSigning] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [a, act] = await Promise.all([api.listApprovals(), api.activity()]);
      setApprovals(a);
      setActivity(act.slice(-8).reverse());
    } catch (e) { setLast(String(e)); }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  async function pursue() {
    if (!intent.trim()) return;
    setBusy(true);
    try {
      await api.createIntent(intent.trim());
      setIntent("");
      setLast("On it.");
      await refresh();
    } catch (e) { setLast(String(e)); }
    setBusy(false);
  }

  async function decideRefuse(id: string) {
    setBusy(true);
    try {
      await api.refuse(id);
      setLast("Refused.");
      await refresh();
    } catch (e) { setLast(String(e)); }
    setBusy(false);
  }

  async function submitApprovalSignature(id: string, b64: string) {
    setSigning(null);
    setBusy(true);
    try {
      await api.approve(id, b64);
      setLast("Signed and approved.");
      await refresh();
      onRefreshMe();
    } catch (e) { setLast(String(e)); }
    setBusy(false);
  }

  if (signing) {
    const a = approvals.find((x) => x.id === signing);
    return (
      <SafeAreaView style={s.root}>
        <StatusBar style="light" />
        <SignatureCapture
          title="Sign to approve"
          subtitle={`${a?.requester ?? "A machine"} wants access. Signing records your approval for this specific request.`}
          onDone={(b64) => submitApprovalSignature(signing, b64)}
          onCancel={() => setSigning(null)}
        />
      </SafeAreaView>
    );
  }

  return (
    <SafeAreaView style={s.root}>
      <StatusBar style="light" />
      <ScrollView
        contentContainerStyle={s.pad}
        refreshControl={<RefreshControl refreshing={false} onRefresh={refresh} />}
      >
        <Text style={s.h1}>What do you want?</Text>
        <TextInput
          value={intent}
          onChangeText={setIntent}
          placeholder="Fix whatever is wrong with HostOS today."
          placeholderTextColor="#666"
          style={s.input}
          editable={!busy}
        />
        <Pressable onPress={pursue} style={[s.btn, busy && s.btnDim]} disabled={busy}>
          <Text style={s.btnText}>pursue</Text>
        </Pressable>

        {last ? <Text style={s.result}>{last}</Text> : null}

        <Text style={s.h2}>Waiting on you</Text>
        {approvals.length === 0 && <Text style={s.dim}>Nothing pending.</Text>}
        {approvals.map((a) => (
          <View key={a.id} style={s.card}>
            <Text style={s.cardTitle}>{a.requester} wants access</Text>
            <Text style={s.dim}>Reason: {a.reason}</Text>
            <Text style={s.dim}>Resources: {a.resources.join(", ")}</Text>
            <Text style={s.dim}>Permissions: {a.permissions.join(", ")}</Text>
            <Text style={s.dim}>Duration: {a.duration}</Text>
            <View style={s.row}>
              <Pressable onPress={() => decideRefuse(a.id)} style={[s.btn, s.btnGhost]} disabled={busy}>
                <Text style={s.btnGhostText}>REFUSE</Text>
              </Pressable>
              <Pressable onPress={() => setSigning(a.id)} style={s.btn} disabled={busy}>
                <Text style={s.btnText}>SIGN & APPROVE</Text>
              </Pressable>
            </View>
          </View>
        ))}

        <Text style={s.h2}>Activity</Text>
        {activity.map((l, i) => <Text key={i} style={s.dim}>{l.kind}</Text>)}
      </ScrollView>
    </SafeAreaView>
  );
}

const s = StyleSheet.create({
  root: { flex: 1, backgroundColor: "#0b0b0c" },
  center: { flex: 1, alignItems: "center", justifyContent: "center", gap: 12, padding: 24 },
  pad: { padding: 20, gap: 12 },
  h1: { color: "#fff", fontSize: 26, fontWeight: "700" },
  h2: { color: "#fff", fontSize: 18, fontWeight: "600", marginTop: 20 },
  input: { color: "#fff", borderWidth: 1, borderColor: "#333", borderRadius: 10, padding: 12 },
  btn: { backgroundColor: "#eee", padding: 12, borderRadius: 10, alignItems: "center" },
  btnDim: { opacity: 0.5 },
  btnGhost: { backgroundColor: "transparent", borderWidth: 1, borderColor: "#444" },
  btnText: { color: "#000", fontWeight: "700" },
  btnGhostText: { color: "#ccc", fontWeight: "700" },
  card: { borderWidth: 1, borderColor: "#2a2a2a", borderRadius: 12, padding: 14, gap: 6 },
  cardTitle: { color: "#fff", fontWeight: "600" },
  dim: { color: "#999" },
  err: { color: "#f66", fontSize: 16, fontWeight: "700" },
  row: { flexDirection: "row", gap: 8, marginTop: 8 },
  result: { color: "#8f8", marginTop: 8 },
});
