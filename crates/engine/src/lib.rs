// Temporary architecture scaffold: implement and verify before the foundation PR merges.
#![allow(dead_code, unused_variables)]
use std::{
    collections::{BTreeMap, BTreeSet},
    num::{NonZeroU16, NonZeroU32},
};
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct At(pub u64); // monotonic ms
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SessionKey(u64); // engine minted
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ActionKey(u64); // engine minted, never reused
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RunKey(u64);
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct BarrierKey(u64);
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct NoticeKey(u64);
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RequestKey(pub u64); // controller minted
// Adapter-local surrogates, NOT raw account/character UIDs, actor IDs, or server request IDs.
macro_rules! keys { ($($name:ident),*) => {$(
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)] pub struct $name(pub u64);
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
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Assisted,
    Clientless,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}
pub struct Boundary {
    map: MapKey,
    center: Point,
    radius: NonZeroU16,
}
impl Boundary {
    pub fn checked(map: MapKey, center: Point, radius: NonZeroU16) -> Result<Self, Refusal> {
        unimplemented!()
    }
    pub fn parts(&self) -> (MapKey, Point, NonZeroU16) {
        unimplemented!()
    }
}
pub struct Percent(u8);
impl Percent {
    pub fn checked(value: u8) -> Result<Self, Refusal> {
        unimplemented!()
    }
    pub fn value(&self) -> u8 {
        self.0
    }
}
pub enum Targets {
    Only(Vec<SpeciesKey>),
    Except(Vec<SpeciesKey>),
}
pub enum Activity {
    Combat { targets: Targets },
    Digging { min_drill_life: u32 },
}
pub struct Goal {
    pub item: ItemKind,
    pub gained: u32,
}
pub struct SetupDraft {
    pub activity: Activity,
    pub boundary: Boundary,
    pub hp_at: Percent,
    pub weight_limit: Percent,
    pub goal: Option<Goal>,
}
pub struct Setup {
    draft: SetupDraft,
} // validated; same activity required while paused
impl Setup {
    pub fn checked(draft: SetupDraft) -> Result<Self, Refusal> {
        unimplemented!()
    }
    pub fn draft(&self) -> &SetupDraft {
        &self.draft
    }
}
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
pub struct Observation {
    pub session: SessionKey,
    pub event: Event,
}
pub enum ConnectStep {
    LoadingGame(u8),
    AwaitingGameLogin,
    SigningIn,
    ChoosingWorld,
    ChoosingCharacter,
    EnteringWorld,
}
pub struct WorldChoice {
    pub key: WorldKey,
    pub label: DisplayText,
}
pub struct CharacterChoice {
    pub key: CharacterKey,
    pub label: DisplayText,
    pub busy: bool,
}
pub struct Support {
    pub build: DisplayText,
    pub move_to: bool,
    pub basic_attack: bool,
    pub dig_cycle: bool,
    pub pickup: bool,
    pub hp_potion: bool,
    pub quiesce: bool,
}
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
    Lost,
    Closed,
    Failed(Halt),
}
pub enum Delivery {
    NotSent,
    Submitted,
    Sent(IngressMark),
    Unknown,
} // local facts, never gameplay confirmation
pub struct Gauge {
    pub current: u32,
    pub maximum: Option<NonZeroU32>,
} // absent maximum != zero
pub struct Profile {
    pub level: u16,
    pub tm_level: u16,
    pub experience: u32,
    pub tm_experience: u32,
    pub galder: u64,
}
pub struct Vitals {
    pub hp: Option<Gauge>,
    pub mp: Option<Gauge>,
    pub weight: Option<Gauge>,
    pub hp_potions: Option<u32>,
}
pub enum Drill {
    Unknown,
    Absent,
    Equipped { life: u32, depth: u32 },
}
pub struct Entity {
    pub key: EntityKey,
    pub species: SpeciesKey,
    pub label: DisplayText,
    pub position: Point,
    pub alive: bool,
    pub hp: Option<Gauge>,
    pub engaged_by_other: Option<bool>,
}
pub struct Drop {
    pub key: DropKey,
    pub kind: ItemKind,
    pub position: Point,
    pub quantity: u32,
}
pub struct Stack {
    pub key: StackKey,
    pub kind: ItemKind,
    pub quantity: u32,
}
pub enum Inventory {
    Reset(Vec<Stack>),
    Put(Stack),
    Remove(StackKey),
    Quantity(StackKey, u32),
}
pub enum Fact {
    OwnLabel(DisplayText),
    Profile(Profile),
    MapLabel { map: MapKey, label: DisplayText },
    Position { map: MapKey, point: Point },
    Hp(Gauge),
    Mp(Gauge),
    Weight(Gauge),
    HpPotions(u32),
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
pub enum DigEnd {
    Empty,
    ItemReported(ItemKind),
} // receipt still needs inventory evidence
pub enum Subject {
    Movement,
    Target(EntityKey),
    Dig,
    Drop(DropKey),
    Potion(StackKey),
}
pub enum Rejection {
    InvalidTarget,
    OutOfRange,
    Resources,
    Forbidden,
    Other,
}
pub enum Outcome {
    Arrived(Point),
    OwnHit(EntityKey),
    OwnKill(EntityKey),
    DigEnded(DigEnd),
    OwnPickup(DropKey),
    PotionUsed(StackKey),
    Rejected { subject: Subject, reason: Rejection },
}
pub enum Action {
    MoveTo(Point),
    BasicAttack(EntityKey),
    DigAt(Point),
    Pickup(DropKey),
    HpPotion(StackKey),
}
pub struct ActionTicket {
    pub id: ActionKey,
    pub action: Action,
    pub not_after: At,
}
pub enum SessionOp {
    ChooseWorld(WorldKey),
    ChooseCharacter(CharacterKey),
    Act(ActionTicket),
    Quiesce(BarrierKey),
    Reconnect { attempt: u8 },
    Close,
}
pub enum Notice {
    Paused(Pause),
    ConnectionLost,
    GoalReached,
}
pub enum Effect {
    Open { session: SessionKey, mode: Mode },
    Session { session: SessionKey, op: SessionOp },
    Notify { key: NoticeKey, notice: Notice },
}
pub struct Effects {
    pub io: Vec<Effect>,
    pub wake_at: Option<At>,
    pub snapshot: Snapshot,
}
pub enum Pause {
    User,
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
pub enum Halt {
    BuildChanged,
    ClientFailed,
    SessionReplaced,
    ActionsRejected,
    MapForbidden,
    ConnectFailed,
}
pub enum Refusal {
    WrongPhase,
    SetupInvalid,
    MissingState,
    Unsupported,
    UnsafeArea,
    PendingAction,
    Busy,
}
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
pub enum SafeLog {
    Sent,
    Confirmed,
    Rejected(Rejection),
    Paused(Pause),
    Halted(Halt),
    Refused(Refusal),
}
pub struct LogEntry {
    pub at: At,
    pub event: SafeLog,
}
// Bounded, plain display text; no Debug/Serialize on observations, snapshots, or handles.
pub struct DisplayText(String);
impl DisplayText {
    pub fn checked(text: String) -> Result<Self, Refusal> {
        unimplemented!()
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
pub struct Snapshot {
    pub mode: Mode,
    pub phase: Phase,
    pub allowed: Vec<Control>,
    pub blocked: Option<Refusal>,
    pub build: Option<DisplayText>,
    pub choices: Choices,
    pub setup: Option<Setup>,
    pub activity: ActivityView,
    pub own: OwnView,
    pub entities: Vec<Entity>,
    pub totals: Totals,
    pub log: Vec<LogEntry>,
}
pub enum DigStep {
    Pick,
    Move,
    Detect,
    Dig,
    Collect,
}
pub enum ActivityView {
    None,
    Combat { target: Option<Entity> },
    Digging { point: Option<Point>, step: DigStep },
}
pub enum Choices {
    None,
    Worlds(Vec<WorldChoice>),
    Characters(Vec<CharacterChoice>),
}
pub struct OwnView {
    pub label: Option<DisplayText>,
    pub profile: Option<Profile>,
    pub map_label: Option<DisplayText>,
    pub position: Option<(MapKey, Point)>,
    pub vitals: Option<Vitals>,
    pub drill: Drill,
    pub last_state_at: Option<At>,
}
pub struct Engine {
    core: Core,
}
impl Engine {
    pub fn new(mode: Mode) -> Self {
        unimplemented!()
    }
    pub fn snapshot(&self) -> Snapshot {
        unimplemented!()
    }
    pub fn handle(&mut self, input: Input) -> Effects {
        unimplemented!()
    }
    // TODO: one transition/guard table derives allowed controls AND command acceptance.
    // TODO: reject stale keys; evidence sequence is stable on redelivery; apply facts then decide <=1 Act.
}
struct Core {
    lifecycle: Lifecycle,
    setup: Option<Setup>,
    book: Book,
    run: Option<Run>,
    support: Option<Support>,
    choices: Choices,
    last_run: Totals,
    log: Vec<LogEntry>,
    refusal: Option<Refusal>,
    quiescing: Option<(BarrierKey, At)>,
    now: At,
    last_request: u64,
    last_evidence: Option<EvidenceKey>,
    next_key: u64,
}
enum Lifecycle {
    Closed(Mode),
    Opening(SessionKey, Mode, ConnectStep),
    Online(SessionKey, Mode),
    Recovering(SessionKey, Mode, u8),
    Disconnected(SessionKey, Mode),
    Closing {
        old: SessionKey,
        mode: Mode,
        next: AfterClose,
    },
    Halted(Option<SessionKey>, Mode, Halt),
}
enum AfterClose {
    Offline(Mode),
    Reopen,
} // fresh auth mints a new SessionKey, preserves held run
struct Book {
    own: OwnView,
    entities: BTreeMap<EntityKey, Entity>,
    bag: BTreeMap<StackKey, Stack>,
    drops: BTreeMap<DropKey, Drop>,
}
struct Run {
    key: RunKey,
    stage: RunStage,
    pending: Option<Pending>,
    rejected: u8,
    totals: Totals,
    killed: BTreeSet<EntityKey>,
    inventory_at_start: BTreeMap<ItemKind, u64>,
}
enum RunStage {
    Starting,
    Running,
    Draining(AfterDrain),
    Paused(Pause),
}
enum AfterDrain {
    Pause(Pause),
    Stop,
}
struct Pending {
    session: SessionKey,
    ticket: ActionTicket,
    floor: EvidenceKey,
    receipt_by: At,
    finish_by: At,
    delivery: Delivery,
    confirmed: bool,
    progress: Progress,
}
enum Progress {
    Awaiting,
    Dig {
        accepted: bool,
        spot: Option<Point>,
        report: Option<DigEnd>,
        bag_before: BTreeMap<ItemKind, u64>,
    },
}
// Receipt <=2s; finish: move/attack/potion/pickup <=5s, dig <=30s (conservative defaults).
// At 15s without validated domain state: drain -> Paused(Uncertain); heartbeat/ACK isn't state.
// Deadline/Unknown: quarantine pending, Quiesce; no verified idle in 2s -> Close, never retry.
// Terminal actions also require a tagged quiescence barrier before the next Act; input-up != idle.
// Pause/Stop drain only the current bounded action; only the expected barrier can finish it.
// Three correlated rejections halt the run; only a confirmed action resets the streak.
// Any reconnect with a held run (resume OR fresh auth) yields Paused(Reconnected); preserve provenance.
// Resume requires fresh baseline + quiescence; late outcomes cannot satisfy a new action.
// SwitchMode refuses any held run, then closes old session; only Closed commits new mode.
