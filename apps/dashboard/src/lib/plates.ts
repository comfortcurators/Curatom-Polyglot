export type PlateId = "knocks" | "keys" | "valhalla" | "billboard" | "activity";

export const PLATES: { id: PlateId; title: string; blurb: string }[] = [
  { id: "knocks", title: "Knocks", blurb: "Machines at the door, right now" },
  { id: "keys", title: "Keys", blurb: "Who holds a door key" },
  { id: "valhalla", title: "Valhalla", blurb: "Sandboxes, open and closed" },
  { id: "billboard", title: "Billboard", blurb: "Every intent and pattern, kept" },
  { id: "activity", title: "Activity", blurb: "The raw feed" },
];
