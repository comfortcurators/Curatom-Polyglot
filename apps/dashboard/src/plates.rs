#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlateId {
    Knocks,
    Keys,
    Valhalla,
    Billboard,
    Activity,
    Account,
    Compute,
    Data,
}

impl PlateId {
    pub const ALL: [PlateId; 8] = [
        PlateId::Knocks,
        PlateId::Keys,
        PlateId::Valhalla,
        PlateId::Billboard,
        PlateId::Activity,
        PlateId::Account,
        PlateId::Compute,
        PlateId::Data,
    ];

    pub fn title(self) -> &'static str {
        match self {
            PlateId::Knocks => "Knocks",
            PlateId::Keys => "Keys",
            PlateId::Valhalla => "Valhalla",
            PlateId::Billboard => "Billboard",
            PlateId::Activity => "Activity",
            PlateId::Account => "Account",
            PlateId::Compute => "Compute",
            PlateId::Data => "Data",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            PlateId::Knocks => "Machines at the door, right now",
            PlateId::Keys => "Who holds a door key",
            PlateId::Valhalla => "Sandboxes, open and closed",
            PlateId::Billboard => "Every intent and pattern, kept",
            PlateId::Activity => "The raw feed",
            PlateId::Account => "Who's signed in, sign out",
            PlateId::Compute => "Your own connectors, your own auth",
            PlateId::Data => "Repository snapshots, synced by hand",
        }
    }

    /// Shared with style.css's .vt-plate-* rules so the browser's native
    /// View Transition morphs this exact tile into this exact panel --
    /// no hand-rolled FLIP animation math.
    pub fn view_transition_class(self) -> &'static str {
        match self {
            PlateId::Knocks => "vt-plate-knocks",
            PlateId::Keys => "vt-plate-keys",
            PlateId::Valhalla => "vt-plate-valhalla",
            PlateId::Billboard => "vt-plate-billboard",
            PlateId::Activity => "vt-plate-activity",
            PlateId::Account => "vt-plate-account",
            PlateId::Compute => "vt-plate-compute",
            PlateId::Data => "vt-plate-data",
        }
    }
}
