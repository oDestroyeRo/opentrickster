use crate::{
    NetError, Result,
    dig::Params,
    profile::{BUILD, verify_hash},
};
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path};

pub(crate) struct GameDefinitions {
    pub(crate) item_types: BTreeMap<u32, (i32, i32)>,
    pub(crate) items: BTreeMap<u32, ItemInfo>,
    pub(crate) labels: BTreeMap<u32, String>,
    pub(crate) drills: BTreeMap<u32, Params>,
    pub(crate) drill_meta: BTreeMap<u32, DrillMeta>,
    pub(crate) gun_params: BTreeMap<u32, GunParams>,
    pub(crate) gun_slots: Vec<usize>,
    pub(crate) zones: BTreeMap<u16, (i32, i32)>,
    pub(crate) zone_labels: BTreeMap<u16, String>,
    pub(crate) normal_form_effects: Vec<i32>,
}
#[derive(Clone, Copy, Deserialize)]
pub(crate) struct DrillMeta {
    #[serde(rename = "Grade")]
    pub(crate) grade: i32,
    #[serde(rename = "DrillLife")]
    pub(crate) maximum_life: i32,
}
#[derive(Clone, Copy, Deserialize)]
pub(crate) struct GunParams {
    #[serde(rename = "GunType")]
    pub(crate) kind: i32,
    #[serde(rename = "GaugeSpeed")]
    pub(crate) speed: i32,
    #[serde(rename = "PowRange")]
    pub(crate) range: i32,
    #[serde(rename = "PowStartPoint")]
    pub(crate) start: i32,
    #[serde(rename = "MinAttackRange")]
    pub(crate) min_range: i32,
    #[serde(rename = "MaxAttackRange")]
    pub(crate) max_range: i32,
}
#[derive(Deserialize)]
pub(crate) struct ItemInfo {
    #[serde(rename = "Weight")]
    pub(crate) weight: i64,
    #[serde(rename = "ItemClass")]
    pub(crate) class: i32,
    #[serde(rename = "Type")]
    pub(crate) kind: i32,
    #[serde(rename = "SubType")]
    pub(crate) subtype: i32,
    #[serde(rename = "Flags")]
    pub(crate) flags: i32,
    #[serde(rename = "FlagsEx")]
    pub(crate) flags_ex: i32,
    #[serde(rename = "MinLevel")]
    pub(crate) min_level: i32,
    #[serde(rename = "Hp")]
    pub(crate) hp: i64,
    #[serde(rename = "Mp")]
    pub(crate) mp: i64,
    #[serde(rename = "HpRecoveryRate")]
    pub(crate) hp_rate: f64,
    #[serde(rename = "MpRecoveryRate")]
    pub(crate) mp_rate: f64,
    #[serde(rename = "Delay")]
    pub(crate) delay: f64,
    #[serde(rename = "Effect")]
    pub(crate) effect: i32,
    #[serde(rename = "ExistType")]
    pub(crate) exist_type: i32,
    #[serde(rename = "Life")]
    pub(crate) life: i32,
    #[serde(rename = "FType")]
    pub(crate) ftype: i32,
}
#[derive(Deserialize)]
struct RawDrill {
    area: i32,
    speed: i32,
    mass: i32,
    limit: i32,
    nature: i32,
}
#[derive(Deserialize)]
struct RawDefinitions {
    build: String,
    item_types: BTreeMap<u32, (i32, i32)>,
    labels: BTreeMap<u32, String>,
    drills: BTreeMap<u32, RawDrill>,
    drill_meta: BTreeMap<u32, DrillMeta>,
    gun_params: BTreeMap<u32, GunParams>,
    gun_slots: Vec<usize>,
    zones: BTreeMap<u16, (i32, i32)>,
    zone_labels: BTreeMap<u16, String>,
    normal_form_effects: Vec<i32>,
    movement_mode: Option<u8>,
}
impl GameDefinitions {
    pub(crate) fn load(cache: &Path) -> Result<Option<Self>> {
        let definitions = cache.join("game_definitions.json");
        let items = cache.join("items.json");
        if !definitions.exists() && !items.exists() {
            return Ok(None);
        }
        let defs = crate::profile::read_bounded(&definitions, 8 * 1024 * 1024)?;
        let items = crate::profile::read_bounded(&items, 24 * 1024 * 1024)?;
        verify_hash(
            &defs,
            "4a7c86ea510f3c2789f07c59715a0f00aae6fe10f93ddc898601b02399a27cc3",
        )?;
        verify_hash(
            &items,
            "50d82abdb872c23e870207b498336bdcd3f58ac59542f9853d311243da3ce654",
        )?;
        Self::parse(&defs, &items).map(Some)
    }
    pub(crate) fn parse(defs: &[u8], items: &[u8]) -> Result<Self> {
        let d: RawDefinitions = serde_json::from_slice(defs).map_err(|_| NetError::Malformed)?;
        let items: BTreeMap<u32, ItemInfo> =
            serde_json::from_slice(items).map_err(|_| NetError::Malformed)?;
        if d.build != BUILD
            || d.movement_mode.is_some()
            || d.item_types.len() != 18118
            || items.len() != 18118
            || d.drills.len() != 191
            || d.gun_params.len() != 611
            || d.zones.len() != 696
            || d.gun_slots.len() != 4
            || d.gun_slots.iter().any(|s| *s > 22)
            || d.normal_form_effects.len() != 6
        {
            return Err(NetError::BuildChanged);
        }
        for (&id, item) in &items {
            if d.item_types.get(&id) != Some(&(item.kind, item.subtype))
                || !item.hp_rate.is_finite()
                || !item.mp_rate.is_finite()
                || !item.delay.is_finite()
            {
                return Err(NetError::Malformed);
            }
        }
        Ok(Self {
            item_types: d.item_types,
            items,
            labels: d.labels,
            drills: d
                .drills
                .into_iter()
                .map(|(id, p)| {
                    (
                        id,
                        Params {
                            area: p.area,
                            speed: p.speed,
                            mass: p.mass,
                            limit: p.limit,
                            nature: p.nature as u32,
                        },
                    )
                })
                .collect(),
            drill_meta: d.drill_meta,
            gun_params: d.gun_params,
            gun_slots: d.gun_slots,
            zones: d.zones,
            zone_labels: d.zone_labels,
            normal_form_effects: d.normal_form_effects,
        })
    }
}
