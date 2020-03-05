use crate::behaviour::Agent;
use crate::{
    world::EntityData,
    world::{Layer, Location},
    World,
};
use ecs::{ComponentTable, Entity};
use grid_2d::CoordIter;
use grid_2d::{Coord, Size};
use procgen::{Sewer, SewerCell, SewerSpec};
use rand::{
    seq::{IteratorRandom, SliceRandom},
    Rng,
};
use rgb24::Rgb24;

pub struct Terrain {
    pub world: World,
    pub player: Entity,
    pub agents: ComponentTable<Agent>,
}

#[allow(dead_code)]
pub fn from_str<R: Rng>(s: &str, player_data: EntityData, rng: &mut R) -> Terrain {
    let rows = s.split('\n').filter(|s| !s.is_empty()).collect::<Vec<_>>();
    let size = Size::new_u16(rows[0].len() as u16, rows.len() as u16);
    let mut world = World::new(size, 0);
    let mut agents = ComponentTable::default();
    let mut player_data = Some(player_data);
    let mut player = None;
    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            if ch.is_control() {
                continue;
            }
            let coord = Coord::new(x as i32, y as i32);
            match ch {
                '.' => {
                    world.spawn_floor(coord);
                }
                'd' => {
                    world.spawn_floor(coord);
                    let entity = world.spawn_slime_divide(coord, rng);
                    agents.insert(entity, Agent::new(size));
                }
                's' => {
                    world.spawn_floor(coord);
                    let entity = world.spawn_slime_swap(coord, rng);
                    agents.insert(entity, Agent::new(size));
                }
                't' => {
                    world.spawn_floor(coord);
                    let entity = world.spawn_slime_teleport(coord, rng);
                    agents.insert(entity, Agent::new(size));
                }
                'g' => {
                    world.spawn_floor(coord);
                    let entity = world.spawn_slime_goo(coord, rng);
                    agents.insert(entity, Agent::new(size));
                }
                'u' => {
                    world.spawn_floor(coord);
                    let entity = world.spawn_slime_attack_upgrade(coord, 0);
                    agents.insert(entity, Agent::new(size));
                }
                'c' => {
                    world.spawn_floor(coord);
                    let entity = world.spawn_slime_curse(coord);
                    agents.insert(entity, Agent::new(size));
                }
                '*' => {
                    world.spawn_floor(coord);
                    world.spawn_light(coord, Rgb24::new(187, 187, 187));
                }
                '#' => {
                    world.spawn_floor(coord);
                    world.spawn_wall(coord);
                }
                '+' => {
                    world.spawn_floor(coord);
                    world.spawn_door(coord);
                }
                '>' => {
                    world.spawn_stairs(coord);
                }
                '~' => {
                    world.spawn_sludge(coord);
                }
                '@' => {
                    world.spawn_floor(coord);
                    let location = Location {
                        coord,
                        layer: Some(Layer::Character),
                    };
                    player = Some(world.insert_entity_data(location, player_data.take().unwrap()));
                }
                'f' => {
                    world.spawn_floor(coord);
                    let entity = world.spawn_former_human(coord);
                    agents.insert(entity, Agent::new(size));
                }
                'h' => {
                    world.spawn_floor(coord);
                    let entity = world.spawn_human(coord);
                    agents.insert(entity, Agent::new(size));
                }
                'A' => {
                    world.spawn_floor(coord);
                    world.spawn_attack(coord, false);
                }
                'D' => {
                    world.spawn_sludge(coord);
                    world.spawn_sludge_light(coord);
                    world.spawn_defend(coord, true);
                }
                'T' => {
                    world.spawn_floor(coord);
                    world.spawn_tech(coord, false);
                }
                _ => log::warn!("unexpected char in terrain: {} ({})", ch.escape_unicode(), ch),
            }
        }
    }
    let player = player.expect("didn't create player");
    Terrain { world, player, agents }
}

#[derive(Clone, Copy)]
enum NpcType {
    Divide,
    Swap,
    Teleport,
    Goo,
}

fn spawn_npc<R: Rng>(world: &mut World, npc_type: NpcType, coord: Coord, rng: &mut R) -> Entity {
    match npc_type {
        NpcType::Divide => world.spawn_slime_divide(coord, rng),
        NpcType::Swap => world.spawn_slime_swap(coord, rng),
        NpcType::Teleport => world.spawn_slime_teleport(coord, rng),
        NpcType::Goo => world.spawn_slime_goo(coord, rng),
    }
}

const ENEMY_TYPES: &[NpcType] = &[
    NpcType::Divide,
    NpcType::Divide,
    NpcType::Divide,
    NpcType::Divide,
    NpcType::Goo,
    NpcType::Goo,
    NpcType::Goo,
    NpcType::Goo,
    NpcType::Swap,
    NpcType::Swap,
    NpcType::Teleport,
];

#[derive(Clone, Copy)]
enum Item {
    Attack,
    Defend,
    Tech,
}

impl Item {
    fn spawn(self, world: &mut World, coord: Coord, special: bool) {
        match self {
            Self::Attack => world.spawn_attack(coord, special),
            Self::Defend => world.spawn_defend(coord, special),
            Self::Tech => world.spawn_tech(coord, special),
        };
    }
}

const ALL_ITEMS: &[Item] = &[Item::Attack, Item::Defend, Item::Tech];

fn sewer_mini<R: Rng>(spec: SewerSpec, player_data: EntityData, rng: &mut R) -> Terrain {
    const MINI_SIZE: Size = Size::new_u16(8, 8);
    let offset = (spec.size.to_coord().unwrap() - MINI_SIZE.to_coord().unwrap()) / 2;
    let mut world = World::new(spec.size, 0);
    let agents = ComponentTable::default();
    let mini_spec = SewerSpec { size: MINI_SIZE };
    let sewer = Sewer::generate(mini_spec, rng);
    for (coord, cell) in sewer.map.enumerate() {
        let coord = coord + offset;
        match cell {
            SewerCell::Wall => {
                world.spawn_wall(coord);
            }
            SewerCell::Floor => {
                world.spawn_floor(coord);
            }
            SewerCell::Door => {
                world.spawn_floor(coord);
                world.spawn_door(coord);
            }
            SewerCell::Pool => {
                world.spawn_sludge(coord);
            }
            SewerCell::Bridge => {
                world.spawn_bridge(coord);
            }
        }
    }
    for coord in CoordIter::new(spec.size) {
        let &cell = world.spatial.get_cell_checked(coord);
        if cell.floor.is_none() && cell.feature.is_none() {
            world.spawn_invisible_wall(coord);
        }
    }
    let player_location = Location {
        coord: sewer.start + offset,
        layer: Some(Layer::Character),
    };
    for light in sewer.lights.iter() {
        world.spawn_sludge_light(light.coord + offset);
    }
    let player = world.insert_entity_data(player_location, player_data);
    world.spawn_stairs(sewer.goal + offset);
    Terrain { world, player, agents }
}

fn sewer_normal<R: Rng>(level: u32, spec: SewerSpec, player_data: EntityData, rng: &mut R) -> Terrain {
    let mut world = World::new(spec.size, level);
    let mut agents = ComponentTable::default();
    let sewer = Sewer::generate(spec, rng);
    let mut npc_candidates = Vec::new();
    for (coord, cell) in sewer.map.enumerate() {
        match cell {
            SewerCell::Wall => {
                world.spawn_wall(coord);
            }
            SewerCell::Floor => {
                world.spawn_floor(coord);
                npc_candidates.push(coord);
            }
            SewerCell::Door => {
                world.spawn_floor(coord);
                world.spawn_door(coord);
            }
            SewerCell::Pool => {
                world.spawn_sludge(coord);
            }
            SewerCell::Bridge => {
                world.spawn_bridge(coord);
            }
        }
    }
    for light in sewer.lights.iter() {
        world.spawn_sludge_light(light.coord);
    }
    world.spawn_stairs(sewer.goal);
    let player_location = Location {
        coord: sewer.start,
        layer: Some(Layer::Character),
    };
    let player = world.insert_entity_data(player_location, player_data);
    let mut empty_coords = sewer
        .map
        .enumerate()
        .filter_map(|(coord, &cell)| {
            if (cell == SewerCell::Bridge || cell == SewerCell::Floor) && coord != sewer.start && coord != sewer.goal {
                Some(coord)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let num_npcs = 4;
    let num_items = 8;
    empty_coords.shuffle(rng);
    for &coord in empty_coords.iter().take(num_npcs) {
        let npc_type = ENEMY_TYPES.choose(rng).unwrap().clone();
        let entity = spawn_npc(&mut world, npc_type, coord, rng);
        agents.insert(entity, Agent::new(spec.size));
    }
    for &coord in empty_coords.iter().skip(num_npcs).take(num_items) {
        let item = ALL_ITEMS.choose(rng).unwrap();
        item.spawn(&mut world, coord, false);
    }
    let num_special_items = 4;
    let special_item_coords = sewer
        .map
        .enumerate()
        .filter_map(
            |(coord, &cell)| {
                if cell == SewerCell::Pool {
                    Some(coord)
                } else {
                    None
                }
            },
        )
        .choose_multiple(rng, num_special_items);
    for (i, &coord) in special_item_coords.iter().enumerate() {
        let item = ALL_ITEMS[i % ALL_ITEMS.len()];
        item.spawn(&mut world, coord, true);
    }
    Terrain { world, player, agents }
}

pub fn sewer<R: Rng>(level: u32, spec: SewerSpec, player_data: EntityData, rng: &mut R) -> Terrain {
    if level == 0 {
        sewer_mini(spec, player_data, rng)
    } else {
        sewer_normal(level, spec, player_data, rng)
    }
}
