use chrono::{self, Utc, NaiveDate, NaiveTime, NaiveDateTime, TimeZone};
use serde_repr::{Serialize_repr, Deserialize_repr};
use serde::{Serialize, Deserialize};
use sqlx::Type;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct Datetime {
    Y: u32, M: u32, D: u32, h: u32, m: u32
}

impl Datetime {
    // Falls back to the Oathbreaker Epoch(tm) if anything goes amiss.
    pub fn to_datetime(&self) -> chrono::DateTime<Utc> {
        let date = NaiveDate::from_ymd_opt(self.Y as i32, self.M, self.D)
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(2021, 5, 2).unwrap());

        let time = NaiveTime::from_hms_opt(self.h, self.m, 0)
            .unwrap_or_else(|| NaiveTime::from_hms_opt(5, 10, 39).unwrap());

        let naive_datetime = NaiveDateTime::new(date, time);

        Utc.from_utc_datetime(&naive_datetime)
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum DurationTag {
    Prm, Ctx, Equ, Tmp,
}

impl DurationTag {
    pub fn is_perm(&self) -> bool {
        match self {
            Self::Prm | Self::Equ => true,
            _ => false,
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            Self::Prm => "Prm",
            Self::Equ => "Equ",
            Self::Ctx => "Ctx",
            Self::Tmp => "Tmp",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Duration {
    pub duration_type: DurationTag,
    pub duration_arg: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatusInfo {
    pub status: String,
    pub power: usize,
    pub duration: Duration,
    pub exhausting: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct Resistances {
    pub Armor: isize,
    pub rFire: isize,
    pub rElec: isize,
    pub rFume: usize,
    pub rAcid: isize,
    pub rHoly: isize,
}

impl Resistances {
    pub const fn placeholder() -> Resistances {
        Resistances { Armor: isize::MAX, rFire: isize::MAX, rElec: isize::MAX, rFume: usize::MAX, rAcid: isize::MAX, rHoly: isize::MAX }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct Stats {
    pub Melee: usize,
    pub Missile: usize,
    pub Martial: usize,
    pub Evade: usize,
    pub Speed: usize,
    pub Vision: usize,
    pub Willpower: usize,
    pub Spikes: usize,
    pub Conjuration: usize,
    pub Potential: usize,
}

impl Stats {
    pub const fn placeholder() -> Stats {
        Stats {
            Melee: usize::MAX,
            Missile: usize::MAX,
            Martial: usize::MAX,
            Evade: usize::MAX,
            Speed: usize::MAX,
            Vision: usize::MAX,
            Willpower: usize::MAX,
            Spikes: usize::MAX,
            Conjuration: usize::MAX,
            Potential: usize::MAX,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub text: String,
    pub dups: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Equipment {
    pub slot_id: String,
    pub slot_name: String,
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DistanceFromStair {
    pub distance: Option<usize>,
    pub is_in_sight: bool,
    pub stair_dest_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MorgueInfo {
    pub seed: u64,
    pub username: String,
    pub end_datetime: Datetime,
    pub turns: u32,

    pub result: String,
    pub slain_str: String,
    pub slain_by_id: String,
    pub slain_by_name: String,
    pub slain_by_captain_id: String,
    pub slain_by_captain_name: String,

    pub level: u8,
    pub statuses: Vec<StatusInfo>,
    #[serde(default = "Stats::placeholder")]
    pub stats: Stats,
    #[serde(default = "Resistances::placeholder")]
    pub resists: Resistances,

    #[serde(default = "placeholders::distance_from_stairs")]
    pub distance_from_stairs: Vec<DistanceFromStair>,
    pub surroundings: Vec<Vec<u32>>,
    pub messages: Vec<Message>,

    pub in_view_ids: Vec<String>,
    pub in_view_names: Vec<String>,
    pub inventory_ids: Vec<String>,
    pub inventory_names: Vec<String>,
    pub equipment: Vec<Equipment>,

    pub aptitudes_names: Vec<String>,
    pub aptitudes_descs: Vec<String>,

    pub augments_names: Vec<String>,
    pub augments_descs: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Value<T = u64> {
    pub floor_type: String,
    pub floor_name: String,
    pub value: T,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValueSet<T = u64> {
    pub total: T,
    pub values: Vec<Value<T>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SingleValueSet<T = u64> {
    #[serde(rename = "type")] _t: Option<String>,
    pub value: ValueSet<T>,
}

impl<T: From<u8>> SingleValueSet<T> {
    pub fn placeholder() -> SingleValueSet<T> {
        Self {
            _t: None,
            value: ValueSet { total: T::from(0), values: Vec::new() },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchValueSetItem {
    pub name: String,
    pub value: ValueSet,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchValueSet {
    #[serde(rename = "type")] _t: Option<String>,
    pub values: Vec<BatchValueSetItem>,
}

impl BatchValueSet {
    pub fn placeholder() -> BatchValueSet {
        Self {
            _t: None,
            values: Vec::new(),
        }
    }

    pub fn total(&self) -> u64 {
        self.values
            .iter()
            .fold(0, |a, set| a + set.value.total)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr, Type)]
#[repr(u16)]
pub enum GameResult {
    Win,
    Lose,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MorgueStats {
    #[serde(rename="turns spent")]       pub turns_spent:         SingleValueSet,
    #[serde(rename="turns w/ statuses")] pub turns_with_statuses: BatchValueSet,
    #[serde(default="SingleValueSet::placeholder")]
    #[serde(rename="health")]            pub health:              SingleValueSet,
    #[serde(default="SingleValueSet::placeholder")]
    #[serde(rename="night reputation")]  pub night_reputation:    SingleValueSet<i64>,
    #[serde(rename="vanquished foes")]   pub vanquished_foes:     BatchValueSet,
    #[serde(rename="stabbed foes")]      pub stabbed_foes:        BatchValueSet,
    #[serde(rename="inflicted damage")]  pub inflicted_damage:    BatchValueSet,
    #[serde(rename="endured damage")]    pub endured_damage:      BatchValueSet,
    #[serde(default="BatchValueSet::placeholder")]
    #[serde(rename="endured spells")]    pub endured_spells:      BatchValueSet,
    #[serde(default="BatchValueSet::placeholder")]
    #[serde(rename="health restored")]   pub endured_healing:     BatchValueSet,
    #[serde(rename="items thrown")]      pub items_thrown:        BatchValueSet,
    #[serde(rename="items used")]        pub items_used:          BatchValueSet,
    #[serde(rename="rings used")]        pub rings_used:          BatchValueSet,
    #[serde(rename="lairs trespassed")]  pub lairs_trespassed:    SingleValueSet,
    #[serde(rename="candles destroyed")] pub candles_destroyed:   SingleValueSet,
    #[serde(rename="shrines drained")]   pub shrines_drained:     SingleValueSet,
    #[serde(rename="times corrupted")]   pub times_corrupted:     BatchValueSet,
    #[serde(rename="wizard keys used")]  pub wizard_keys_used:    BatchValueSet,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Morgue {
    pub info: MorgueInfo,
    pub stats: MorgueStats,
}

impl Morgue {
    pub fn result(&self) -> GameResult {
        if self.info.result.starts_with("Faced the ") // Face the Necromancer's wrath
        || self.info.result.starts_with("Paid for ") // Paid for their treachery
        {
            GameResult::Lose
        } else if self.info.result.starts_with("Escaped") {
            GameResult::Win
        } else {
            GameResult::Unknown
        }
    }
}

pub mod placeholders {
    use super::*;
    pub fn distance_from_stairs() -> Vec<DistanceFromStair> { Vec::new() }
}
