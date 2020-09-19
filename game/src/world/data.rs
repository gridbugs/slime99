use crate::visibility::Light;
pub use crate::world::{
    explosion_spec,
    player::{AbilityTarget, Player},
    spatial::{Layer, Location},
};
use direction::CardinalDirection;
use entity_table::declare_entity_module;
use rgb24::Rgb24;
use serde::{Deserialize, Serialize};

declare_entity_module! {
    components {
        tile: Tile,
        opacity: u8,
        solid: (),
        realtime: (),
        blocks_gameplay: (),
        light: Light,
        on_collision: OnCollision,
        colour_hint: Rgb24,
        npc: Npc,
        character: (),
        collides_with: CollidesWith,
        projectile_damage: ProjectileDamage,
        hit_points: HitPoints,
        blood: (),
        player: Player,
        ignore_lighting: (),
        door_state: DoorState,
        stairs: (),
        next_action: NpcAction,
        to_remove: (),
        sludge: (),
        safe_on_sludge: (),
        on_damage: OnDamage,
        move_half_speed: MoveHalfSpeed,
        item: Item,
        drop_item_on_death: DropItemOnDeath,
    }
}
pub use components::Components;
pub use components::EntityData;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tile {
    Player,
    Wall,
    Floor,
    DoorClosed,
    DoorOpen,
    Stairs,
    Sludge0,
    Sludge1,
    Bridge,
    SlimeDivide,
    SlimeTeleport,
    SlimeSwap,
    SlimeGoo,
    SlimeCurse,
    SlimeAttackUpgrade,
    SlimeDefendUpgrade,
    SlimeTechUpgrade,
    SlimeBoss,
    AttackItem { special: bool },
    DefendItem { special: bool },
    TechItem { special: bool },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Item {
    Attack { special: bool },
    Defend { special: bool },
    Tech { special: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Disposition {
    Hostile,
    Afraid,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Npc {
    pub disposition: Disposition,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OnCollision {
    Explode(explosion_spec::Explosion),
    Remove,
    RemoveRealtime,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CollidesWith {
    pub solid: bool,
    pub character: bool,
}

impl Default for CollidesWith {
    fn default() -> Self {
        Self {
            solid: true,
            character: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ProjectileDamage {
    pub hit_points: u32,
    pub push_back: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HitPoints {
    pub current: u32,
    pub max: u32,
}

impl HitPoints {
    pub fn new_full(max: u32) -> Self {
        Self { current: max, max }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DoorState {
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NpcAction {
    Walk(CardinalDirection),
    Wait,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OnDamage {
    Sludge,
    Divide,
    DivideAndSpawn,
    Teleport,
    Swap,
    Upgrade {
        level: u32,
        ability_target: AbilityTarget,
    },
    Curse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MoveHalfSpeed {
    pub skip_next_move: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DropItemOnDeath {
    GuaranteeSpecial,
    RandomNormal,
}
