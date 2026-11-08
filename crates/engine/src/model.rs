use std::num::{NonZeroU16, NonZeroU32};
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct At(pub u64); // monotonic ms
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SessionKey(pub(crate) u64); // engine minted
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActionKey(pub(crate) u64); // engine minted, never reused
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RunKey(pub(crate) u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BarrierKey(pub(crate) u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NoticeKey(pub(crate) u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestKey(pub u64); // controller minted
// Adapter-local surrogates, NOT raw account/character UIDs, actor IDs, or server request IDs.
macro_rules! keys { ($($name:ident),*) => {$(
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)] pub struct $name(pub u64);
)*}; }
keys!(
    IngressMark,
    EvidenceKey,
    EntityKey,
    StackKey,
    DropKey,
    MapKey,
    SpeciesKey,
    ItemKind,
    WorldKey,
    CharacterKey
);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Assisted,
    Clientless,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}
#[derive(Clone)]
pub struct Boundary {
    map: MapKey,
    center: Point,
    radius: NonZeroU16,
}
impl Boundary {
    pub fn checked(map: MapKey, center: Point, radius: NonZeroU16) -> Result<Self, Refusal> {
        let radius = i32::from(radius.get());
        if !wire_point(center)
            || center.x - radius < 0
            || center.y - radius < 0
            || center.x + radius > i32::from(u16::MAX)
            || center.y + radius > i32::from(u16::MAX)
        {
            return Err(Refusal::UnsafeArea);
        }
        Ok(Self {
            map,
            center,
            radius: NonZeroU16::new(radius as u16).expect("positive radius"),
        })
    }
    pub fn parts(&self) -> (MapKey, Point, NonZeroU16) {
        (self.map, self.center, self.radius)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Percent(u8);
impl Percent {
    pub fn checked(value: u8) -> Result<Self, Refusal> {
        if value > 100 {
            Err(Refusal::SetupInvalid)
        } else {
            Ok(Self(value))
        }
    }
    pub fn value(&self) -> u8 {
        self.0
    }
}
#[derive(Clone)]
pub enum Targets {
    Only(Vec<SpeciesKey>),
    Except(Vec<SpeciesKey>),
}
#[derive(Clone)]
pub enum Activity {
    Combat { targets: Targets },
    Digging { min_drill_life: u32 },
}
#[derive(Clone)]
pub struct Goal {
    pub item: ItemKind,
    pub gained: u32,
}
#[derive(Clone)]
pub struct SetupDraft {
    pub activity: Activity,
    pub boundary: Boundary,
    pub hp_at: Percent,
    pub weight_limit: Percent,
    pub goal: Option<Goal>,
}
#[derive(Clone)]
pub struct Setup {
    draft: SetupDraft,
} // validated; same activity required while paused
impl Setup {
    pub fn checked(draft: SetupDraft) -> Result<Self, Refusal> {
        if draft.goal.as_ref().is_some_and(|goal| goal.gained == 0)
            || matches!(&draft.activity, Activity::Combat { targets: Targets::Only(targets) } if targets.is_empty())
            || matches!(&draft.activity, Activity::Digging { min_drill_life: 0 })
            || draft.weight_limit.value() == 0
        {
            return Err(Refusal::SetupInvalid);
        }
        Ok(Self { draft })
    }
    pub fn draft(&self) -> &SetupDraft {
        &self.draft
    }
}
#[derive(Clone)]
pub enum Command {
    Open,
    CancelConnect,
    ChooseWorld(WorldKey),
    ChooseCharacter(CharacterKey),
    Configure(Setup),
    Start,
    Pause,
    Resume,
    Stop,
    Reconnect,
    DismissHalt,
    SwitchMode(Mode),
}
#[derive(Clone)]
pub enum Input {
    User {
        at: At,
        request: RequestKey,
        command: Command,
    },
    Observed {
        at: At,
        observation: Observation,
    },
    Wake(At),
}
#[derive(Clone)]
pub struct Observation {
    pub session: SessionKey,
    pub event: Event,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectStep {
    LoadingGame(u8),
    AwaitingGameLogin,
    SigningIn,
    ChoosingWorld,
    ChoosingCharacter,
    EnteringWorld,
}
#[derive(Clone)]
pub struct WorldChoice {
    pub key: WorldKey,
    pub label: DisplayText,
}
#[derive(Clone)]
pub struct CharacterChoice {
    pub key: CharacterKey,
    pub label: DisplayText,
    pub busy: bool,
}
#[derive(Clone)]
pub struct Support {
    pub build: DisplayText,
    pub move_to: bool,
    pub basic_attack: bool,
    pub dig_cycle: bool,
    pub pickup: bool,
    pub hp_potion: bool,
    pub quiesce: bool,
}
#[derive(Clone)]
pub enum Event {
    Progress(ConnectStep),
    Worlds(Vec<WorldChoice>),
    Characters(Vec<CharacterChoice>),
    Baseline {
        support: Support,
        revision: EvidenceKey,
        facts: Vec<Fact>,
        resumed: bool,
    },
    Facts {
        revision: EvidenceKey,
        ingress: IngressMark,
        facts: Vec<Fact>,
    },
    Receipt {
        action: ActionKey,
        delivery: Delivery,
    },
    Quiescent(BarrierKey),
    /// Host fences queued gestures before physical input, then reports this event.
    ManualInput,
    Lost,
    Closed,
    Failed(Halt),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    NotSent,
    Submitted,
    Sent(IngressMark),
    Unknown,
} // local facts, never gameplay confirmation
#[derive(Clone)]
pub struct Gauge {
    pub current: u32,
    pub maximum: Option<NonZeroU32>,
} // absent maximum != zero
#[derive(Clone)]
pub struct Profile {
    pub level: u16,
    pub tm_level: u16,
    pub experience: u32,
    pub tm_experience: u32,
    pub galder: u64,
}
#[derive(Clone)]
pub struct Vitals {
    pub hp: Option<Gauge>,
    pub mp: Option<Gauge>,
    pub weight: Option<Gauge>,
    pub hp_potions: Option<u32>,
}
#[derive(Clone)]
pub enum Drill {
    Unknown,
    Absent,
    Equipped { life: u32, depth: u32 },
}
#[derive(Clone)]
pub struct Entity {
    pub key: EntityKey,
    pub species: SpeciesKey,
    pub label: DisplayText,
    pub position: Point,
    pub alive: bool,
    pub hp: Option<Gauge>,
    pub engaged_by_other: Option<bool>,
}
#[derive(Clone)]
pub struct Drop {
    pub key: DropKey,
    pub kind: ItemKind,
    pub position: Point,
    pub quantity: u32,
}
#[derive(Clone)]
pub struct Stack {
    pub key: StackKey,
    pub kind: ItemKind,
    pub quantity: u32,
}
#[derive(Clone)]
pub enum Inventory {
    Reset(Vec<Stack>),
    Put(Stack),
    Remove(StackKey),
    Quantity(StackKey, u32),
}
#[derive(Clone)]
pub enum Fact {
    OwnLabel(DisplayText),
    Profile(Profile),
    MapLabel {
        map: MapKey,
        label: DisplayText,
    },
    Position {
        map: MapKey,
        point: Point,
    },
    Hp(Gauge),
    Mp(Gauge),
    Weight(Gauge),
    HpPotions(u32),
    /// Adapter-verified usable HP potion stack; never infer this from an item number.
    HpPotionStack(Option<StackKey>),
    Drill(Drill),
    Drop(Drop),
    DropGone(DropKey),
    Entity(Entity),
    EntityGone(EntityKey),
    Inventory(Inventory),
    DigAccepted,
    DigSpot(Point),
    Outcome(Outcome),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigEnd {
    Empty,
    ItemReported(ItemKind),
} // receipt still needs inventory evidence
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subject {
    Movement,
    Target(EntityKey),
    Dig,
    Drop(DropKey),
    Potion(StackKey),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    InvalidTarget,
    OutOfRange,
    Resources,
    Forbidden,
    Other,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Arrived(Point),
    OwnHit(EntityKey),
    OwnKill(EntityKey),
    DigEnded(DigEnd),
    OwnPickup(DropKey),
    PotionUsed(StackKey),
    Rejected { subject: Subject, reason: Rejection },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    MoveTo(Point),
    BasicAttack(EntityKey),
    DigAt(Point),
    Pickup(DropKey),
    HpPotion(StackKey),
}
#[derive(Clone)]
pub struct ActionTicket {
    pub id: ActionKey,
    pub action: Action,
    pub not_after: At,
}
#[derive(Clone)]
pub enum SessionOp {
    ChooseWorld(WorldKey),
    ChooseCharacter(CharacterKey),
    Act(ActionTicket),
    Quiesce(BarrierKey),
    Reconnect { attempt: u8 },
    Close,
}
#[derive(Clone)]
pub enum Notice {
    Paused(Pause),
    ConnectionLost,
    GoalReached,
}
#[derive(Clone)]
pub enum Effect {
    Open { session: SessionKey, mode: Mode },
    Session { session: SessionKey, op: SessionOp },
    Notify { key: NoticeKey, notice: Notice },
}
#[derive(Clone)]
pub struct Effects {
    pub io: Vec<Effect>,
    pub wake_at: Option<At>,
    pub snapshot: Snapshot,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pause {
    User,
    ManualInput,
    HpPotions,
    Weight,
    Drill,
    Depth,
    Goal,
    Fainted,
    NoDrops,
    Uncertain,
    Reconnected,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Halt {
    BuildChanged,
    ClientFailed,
    SessionReplaced,
    ActionsRejected,
    MapForbidden,
    ConnectFailed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    WrongPhase,
    SetupInvalid,
    MissingState,
    Unsupported,
    UnsafeArea,
    PendingAction,
    Busy,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Open,
    CancelConnect,
    Choose,
    Configure,
    Start,
    Pause,
    Resume,
    Stop,
    Reconnect,
    DismissHalt,
    SwitchMode,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    ClientAbsent,
    ClientLoading,
    AwaitingLogin,
    Offline,
    Connecting,
    Ready,
    Starting,
    Running,
    Pausing,
    Paused(Pause),
    Stopping,
    Reconnecting(u8),
    Disconnected,
    Closing,
    Error(Halt),
}
#[derive(Clone)]
pub struct Totals {
    pub sent: u64,
    pub confirmed: u64,
    pub kills: u64,
    pub digs: u64,
    pub finds: u64,
    pub empty: u64,
    pub goal_gained: u64,
    pub elapsed_ms: u64,
}
#[derive(Clone)]
pub enum SafeLog {
    Sent,
    Confirmed,
    Rejected(Rejection),
    Paused(Pause),
    Halted(Halt),
    Refused(Refusal),
}
#[derive(Clone)]
pub struct LogEntry {
    pub at: At,
    pub event: SafeLog,
}
// Bounded, plain display text; no Debug/Serialize on observations, snapshots, or handles.
#[derive(Clone)]
pub struct DisplayText(String);
impl DisplayText {
    pub fn checked(text: String) -> Result<Self, Refusal> {
        if text.is_empty() || text.len() > 256 || text.chars().any(char::is_control) {
            Err(Refusal::SetupInvalid)
        } else {
            Ok(Self(text))
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Clone)]
pub struct Snapshot {
    pub mode: Mode,
    pub phase: Phase,
    pub allowed: Vec<Control>,
    pub blocked: Option<Refusal>,
    /// Each blocker names the observable condition required before Start or Resume.
    pub blockers: Vec<Blocker>,
    pub build: Option<DisplayText>,
    pub choices: Choices,
    pub setup: Option<Setup>,
    pub activity: ActivityView,
    pub own: OwnView,
    pub entities: Vec<Entity>,
    /// Received stacks only; an empty view with unknown inventory is not an empty bag.
    pub inventory: Vec<Stack>,
    pub totals: Totals,
    pub log: Vec<LogEntry>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigStep {
    Pick,
    Move,
    Detect,
    Dig,
    Collect,
}
#[derive(Clone)]
pub enum ActivityView {
    None,
    Combat { target: Option<Entity> },
    Digging { point: Option<Point>, step: DigStep },
}
#[derive(Clone)]
pub enum Choices {
    None,
    Worlds(Vec<WorldChoice>),
    Characters(Vec<CharacterChoice>),
}
#[derive(Clone)]
pub struct OwnView {
    pub label: Option<DisplayText>,
    pub profile: Option<Profile>,
    pub map_label: Option<DisplayText>,
    pub position: Option<(MapKey, Point)>,
    pub vitals: Option<Vitals>,
    pub drill: Drill,
    pub last_state_at: Option<At>,
    pub inventory: InventoryKnowledge,
}

/// Coordinates are unsigned 16-bit game-coordinate counts, never screen pixels.
pub(crate) fn wire_point(point: Point) -> bool {
    (0..=i32::from(u16::MAX)).contains(&point.x) && (0..=i32::from(u16::MAX)).contains(&point.y)
}
impl Boundary {
    pub(crate) fn contains(&self, map: MapKey, point: Point) -> bool {
        self.map == map && distance(self.center, point) <= u64::from(self.radius.get()).pow(2)
    }
}
pub(crate) fn distance(a: Point, b: Point) -> u64 {
    let x = i64::from(a.x) - i64::from(b.x);
    let y = i64::from(a.y) - i64::from(b.y);
    (i128::from(x).pow(2) + i128::from(y).pow(2)).min(i128::from(u64::MAX)) as u64
}

impl SessionKey {
    /// The local capability value, never a server identifier.
    pub fn value(self) -> u64 {
        self.0
    }
}

impl ActionKey {
    /// The local capability value, never a server identifier.
    pub fn value(self) -> u64 {
        self.0
    }
}

impl RunKey {
    /// The local capability value, never a server identifier.
    pub fn value(self) -> u64 {
        self.0
    }
}

impl BarrierKey {
    /// The local capability value, never a server identifier.
    pub fn value(self) -> u64 {
        self.0
    }
}

impl NoticeKey {
    /// The local capability value, never a server identifier.
    pub fn value(self) -> u64 {
        self.0
    }
}

/// Observed means inventory data was received, never that an absent packet was an empty bag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryKnowledge {
    Unknown,
    Observed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateKind {
    WorldState,
    Position,
    Hp,
    HpMaximum,
    HpPotions,
    HpPotionStack,
    Weight,
    WeightMaximum,
    Inventory,
    Drill,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Movement,
    BasicAttack,
    DigCycle,
    Pickup,
    HpPotion,
    Quiescence,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocker {
    NeedsConnection,
    NeedsSetup,
    NeedsQuiescence,
    NeedsActionDrain,
    NeedsObservation(StateKind),
    NeedsFreshObservation(StateKind),
    NeedsCapability(Capability),
    NeedsBoundaryMap(MapKey),
    NeedsInsideBoundary,
    NeedsHpAbove { percent: u8 },
    NeedsWeightBelow { percent: u8 },
    NeedsLivingCharacter,
    NeedsHpPotions,
    NeedsEquippedDrill,
    NeedsDrillLife { minimum: u32 },
    NeedsDrillDepth,
    GoalReached { gained: u64, target: u32 },
}
