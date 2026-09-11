import React, { useEffect, useState, useCallback } from "react";
import { SafeAreaView, ScrollView, View, Text, TextInput, Pressable, StyleSheet, RefreshControl } from "react-native";
import { StatusBar } from "expo-status-bar";
import * as api from "./lib/api";

type ApprovalView = {
  id: string;
  requester: string;
  reason: string;
  resources: string[];
  permissions: string[];
  duration: string;
};

export default function App() {
  const [intent, setIntent] = useState("");
  const [approvals, setApprovals] = useState<ApprovalView[]>([]);
  const [last, setLast] = useState<string>("");
  const [activity, setActivity] = useState<{ kind: string; summary?: string }[]>([]);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const [a, act] = await Promise.all([api.listApprovals(), api.activity()]);
      setApprovals(a);
      setActivity(act.slice(-8).reverse());
    } catch (e) {
      setLast(String(e));
    }
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

  async function decide(id: string, decision: "approve" | "refuse") {
    setBusy(true);
    try {
      if (decision === "approve") {
        const r = await api.approve(id, "sign2:ok"); // dev SIGN2
        const summary = r?.outcomes?.[0]?.data?._mock
          ? "[MOCK] HostOS is healthy. I found 7 services."
          : "Done.";
        setLast(summary);
      } else {
        await api.refuse(id);
        setLast("Refused.");
      }
      await refresh();
    } catch (e) { setLast(String(e)); }
    setBusy(false);
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
              <Pressable onPress={() => decide(a.id, "refuse")} style={[s.btn, s.btnGhost]} disabled={busy}>
                <Text style={s.btnGhostText}>REFUSE</Text>
              </Pressable>
              <Pressable onPress={() => decide(a.id, "approve")} style={s.btn} disabled={busy}>
                <Text style={s.btnText}>APPROVE</Text>
              </Pressable>
            </View>
          </View>
        ))}

        <Text style={s.h2}>Activity</Text>
        {activity.map((l, i) => <Text key={i} style={s.dim}>{l.summary ?? l.kind}</Text>)}
      </ScrollView>
    </SafeAreaView>
  );
}

const s = StyleSheet.create({
  root: { flex: 1, backgroundColor: "#0b0b0c" },
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
  row: { flexDirection: "row", gap: 8, marginTop: 8 },
  result: { color: "#8f8", marginTop: 8 },
});
