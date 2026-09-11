import React, { useCallback, useEffect, useState } from "react";
import {
  SafeAreaView, ScrollView, View, Text, TextInput, Pressable, StyleSheet,
  RefreshControl, ActivityIndicator, Modal,
} from "react-native";
import { StatusBar } from "expo-status-bar";
import * as api from "./lib/api";
import EnrollmentScreen from "./screens/EnrollmentScreen";

type ApprovalView = Awaited<ReturnType<typeof api.listApprovals>>[number];

export default function App() {
  const [me, setMe] = useState<api.Me | null>(null);
  const [bootError, setBootError] = useState<string | null>(null);

  const refreshMe = useCallback(async () => {
    try {
      const m = await api.me();
      setMe(m);
      setBootError(null);
    } catch (e) {
      setBootError(String(e));
    }
  }, []);

  useEffect(() => { refreshMe(); }, [refreshMe]);

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

  return <Main />;
}

function Main() {
  const [intent, setIntent] = useState("");
  const [approvals, setApprovals] = useState<ApprovalView[]>([]);
  const [last, setLast] = useState("");
  const [activity, setActivity] = useState<{ kind: string }[]>([]);
  const [busy, setBusy] = useState(false);
  const [confirming, setConfirming] = useState<ApprovalView | null>(null);

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

  async function confirmApprove() {
    if (!confirming) return;
    const id = confirming.id;
    setConfirming(null);
    setBusy(true);
    try {
      await api.approve(id);
      setLast("Approved. Working on it.");
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
              <Pressable onPress={() => decideRefuse(a.id)} style={[s.btn, s.btnGhost]} disabled={busy}>
                <Text style={s.btnGhostText}>REFUSE</Text>
              </Pressable>
              <Pressable onPress={() => setConfirming(a)} style={s.btn} disabled={busy}>
                <Text style={s.btnText}>APPROVE</Text>
              </Pressable>
            </View>
          </View>
        ))}

        <Text style={s.h2}>Activity</Text>
        {activity.map((l, i) => <Text key={i} style={s.dim}>{l.kind}</Text>)}
      </ScrollView>

      <Modal
        visible={confirming !== null}
        transparent
        animationType="fade"
        onRequestClose={() => setConfirming(null)}
      >
        <View style={s.modalBackdrop}>
          <View style={s.modalCard}>
            <Text style={s.modalH}>Confirm approval</Text>
            {confirming ? (
              <>
                <Text style={s.modalText}>
                  {confirming.requester} is asking to {confirming.permissions.join(", ")}{" "}
                  {confirming.resources.join(", ")}.
                </Text>
                <Text style={s.modalDim}>
                  Reason: {confirming.reason}
                </Text>
                <Text style={s.modalDim}>
                  This approval is single-use. It expires after one execution.
                </Text>
              </>
            ) : null}
            <View style={s.row}>
              <Pressable onPress={() => setConfirming(null)} style={[s.btn, s.btnGhost]}>
                <Text style={s.btnGhostText}>CANCEL</Text>
              </Pressable>
              <Pressable onPress={confirmApprove} style={s.btn}>
                <Text style={s.btnText}>CONFIRM</Text>
              </Pressable>
            </View>
          </View>
        </View>
      </Modal>
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
  btn: { backgroundColor: "#eee", padding: 12, borderRadius: 10, alignItems: "center", flex: 1 },
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
  modalBackdrop: { flex: 1, backgroundColor: "rgba(0,0,0,0.7)", justifyContent: "center", padding: 24 },
  modalCard: { backgroundColor: "#141416", borderRadius: 14, padding: 20, gap: 12 },
  modalH: { color: "#fff", fontSize: 18, fontWeight: "700" },
  modalText: { color: "#ccc", fontSize: 15, lineHeight: 22 },
  modalDim: { color: "#888", fontSize: 13, lineHeight: 20 },
});
