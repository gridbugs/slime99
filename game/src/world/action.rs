use crate::{
    world::{
        data::{DoorState, DropItemOnDeath, Item, OnCollision, OnDamage, ProjectileDamage, Tile},
        explosion, player,
        realtime_periodic::{core::ScheduledRealtimePeriodicState, movement},
        spatial::Spatial,
        spatial::{Layer, Location, OccupiedBy},
        ExternalEvent, World,
    },
    VisibilityGrid,
};
use entity_table::Entity;
use direction::{CardinalDirection, Direction};
use grid_2d::Coord;
use rand::{seq::IteratorRandom, seq::SliceRandom, Rng};
use std::collections::{HashSet, VecDeque};
use std::time::Duration;

#[derive(Clone, Copy)]
pub enum Error {
    BlinkToNonVisibleCell,
    BlinkToSolidCell,
    NoTechToApply,
    BlinkWithoutDestination,
    AttackDeckFull,
    DefendDeckFull,
    WalkIntoSolidCell,
    NoAbilityInSlot,
    NotEnoughAttacks,
    NotEnoughDefends,
    NotEnoughTechs,
}

impl World {
    pub fn apply_ability<R: Rng>(&mut self, entity: Entity, ability_slot: u8, rng: &mut R) -> Result<(), Error> {
        let player = self.components.player.get_mut(entity).unwrap();
        if let Some(ability) = player.ability.get(ability_slot as usize) {
            use player::{Ability::*, AbilityTarget::*};
            match ability {
                SwapTop2(Attack) => player.attack.swap_top_2().map_err(|_| Error::NotEnoughAttacks)?,
                SwapTop2(Defend) => player.defend.swap_top_2().map_err(|_| Error::NotEnoughDefends)?,
                SwapTop2(Tech) => player.tech.swap_top_2().map_err(|_| Error::NotEnoughTechs)?,
                Stash(Attack) => player.attack.stash().map_err(|_| Error::NotEnoughAttacks)?,
                Stash(Defend) => player.defend.stash().map_err(|_| Error::NotEnoughDefends)?,
                Stash(Tech) => player.tech.stash().map_err(|_| Error::NotEnoughTechs)?,
                Discard(Attack) => {
                    player.attack.pop().ok_or_else(|| Error::NotEnoughAttacks)?;
                }
                Discard(Defend) => {
                    player.defend.pop().ok_or_else(|| Error::NotEnoughDefends)?;
                }
                Discard(Tech) => {
                    player.tech.pop().ok_or_else(|| Error::NotEnoughTechs)?;
                }
            }
            self.wait(entity, rng);
        } else {
            return Err(Error::NoAbilityInSlot);
        }
        Ok(())
    }

    pub fn wait<R: Rng>(&mut self, entity: Entity, rng: &mut R) {
        if let Some(coord) = self.spatial.coord(entity) {
            self.after_player_move(entity, coord, rng);
        }
    }
    fn pick_up_item<R: Rng>(&mut self, character: Entity, item_entity: Entity, rng: &mut R) {
        if self.components.to_remove.contains(character) {
            return;
        }
        let player = self.components.player.get_mut(character).unwrap();
        if let Some(item) = self.components.item.get(item_entity) {
            let taken = match item {
                Item::Attack { special } => {
                    if player.attack.is_full() {
                        false
                    } else {
                        let attack = player::choose_attack(self.level, *special, rng);
                        let _ = player.attack.push(attack);
                        true
                    }
                }
                Item::Defend { special } => {
                    if player.defend.is_full() {
                        false
                    } else {
                        let defend = player::choose_defend(self.level, *special, rng);
                        let _ = player.defend.push(defend);
                        true
                    }
                }
                Item::Tech { special } => {
                    if player.tech.is_full() {
                        false
                    } else {
                        let tech = player::choose_tech(self.level, *special, rng);
                        let _ = player.tech.push(tech);
                        true
                    }
                }
            };
            if taken {
                self.components.to_remove.insert(item_entity, ());
            }
        }
    }
    fn after_player_move<R: Rng>(&mut self, character: Entity, target_coord: Coord, rng: &mut R) {
        if let Some(&cell) = self.spatial.get_cell(target_coord) {
            if let Some(floor_entity) = cell.floor {
                if self.components.sludge.contains(floor_entity) {
                    self.apply_defend(character, rng);
                }
            }
            if let Some(feature_entity) = cell.feature {
                if self.components.item.contains(feature_entity) {
                    self.pick_up_item(character, feature_entity, rng);
                }
            }
        }
    }
    pub fn character_walk_in_direction<R: Rng>(
        &mut self,
        character: Entity,
        direction: CardinalDirection,
        rng: &mut R,
    ) -> Result<(), Error> {
        if let Some(move_half_speed) = self.components.move_half_speed.get_mut(character) {
            if move_half_speed.skip_next_move {
                move_half_speed.skip_next_move = false;
                return Ok(());
            }
            move_half_speed.skip_next_move = true;
        }
        let current_coord = if let Some(coord) = self.spatial.coord(character) {
            coord
        } else {
            panic!("failed to find coord for {:?}", character);
        };
        let target_coord = current_coord + direction.coord();
        if let Some(&cell) = self.spatial.get_cell(target_coord) {
            if let Some(feature_entity) = cell.feature {
                if self.components.solid.contains(feature_entity) {
                    if let Some(DoorState::Closed) = self.components.door_state.get(feature_entity).cloned() {
                        self.open_door(feature_entity);
                    } else {
                        return Err(Error::WalkIntoSolidCell);
                    }
                }
            }
        } else {
            return Err(Error::WalkIntoSolidCell);
        }
        if let Err(OccupiedBy(occupant)) = self.spatial.update_coord(character, target_coord) {
            self.melee_attack(character, occupant, direction, rng);
        } else {
            if self.components.player.contains(character) {
                self.after_player_move(character, target_coord, rng);
            }
        }
        Ok(())
    }

    pub fn grant_ability(&mut self, entity: Entity, ability: player::Ability) {
        let player = self.components.player.get_mut(entity).unwrap();
        let _ = player.ability.push(ability);
    }

    fn player_melee_attack<R: Rng>(
        &mut self,
        attacker: Entity,
        victim: Entity,
        direction: CardinalDirection,
        rng: &mut R,
    ) {
        let player = self.components.player.get_mut(attacker).unwrap();
        let attack = player.attack.pop().unwrap_or(player::EMPTY_ATTACK);
        self.apply_attack(attack, attacker, victim, direction, rng);
        self.wait(attacker, rng);
    }

    fn npc_melee_attack<R: Rng>(&mut self, _attacker: Entity, victim: Entity, rng: &mut R) {
        self.apply_defend(victim, rng);
    }

    fn cleave<R: Rng>(&mut self, entity: Entity, damage: u32, rng: &mut R) {
        let this_coord = self.spatial.coord(entity).unwrap();
        for direction in Direction::all() {
            let coord = this_coord + direction.coord();
            if let Some(cell) = self.spatial.get_cell(coord) {
                if let Some(entity) = cell.character {
                    self.damage_character(entity, damage, rng);
                }
            }
        }
    }

    fn skewer<R: Rng>(&mut self, entity: Entity, damage: u32, direction: CardinalDirection, rng: &mut R) {
        const RANGE: u32 = 4;
        let mut coord = self.spatial.coord(entity).unwrap();
        for _ in 0..RANGE {
            coord += direction.coord();
            if let Some(cell) = self.spatial.get_cell(coord) {
                if cell.feature.is_some() {
                    break;
                }
                if let Some(entity) = cell.character {
                    self.damage_character(entity, damage, rng);
                }
            }
        }
    }

    fn apply_attack<R: Rng>(
        &mut self,
        attack: player::Attack,
        attacker: Entity,
        victim: Entity,
        direction: CardinalDirection,
        rng: &mut R,
    ) {
        use player::Attack::*;
        match attack {
            Miss => (),
            Hit(n) => self.damage_character(victim, n, rng),
            Cleave(n) => self.cleave(attacker, n, rng),
            Skewer(n) => self.skewer(attacker, n, direction, rng),
        }
    }

    fn teleport<R: Rng>(&mut self, entity: Entity, rng: &mut R) {
        let maybe_coord = self
            .spatial
            .enumerate()
            .filter_map(|(coord, cell)| {
                if let Some(floor_entity) = cell.floor {
                    if self.components.sludge.contains(floor_entity) {
                        None
                    } else {
                        if cell.character.is_none() && cell.feature.is_none() {
                            Some(coord)
                        } else {
                            None
                        }
                    }
                } else {
                    None
                }
            })
            .choose(rng);
        if let Some(coord) = maybe_coord {
            self.spatial.update_coord(entity, coord).unwrap();
        }
    }

    fn revenge<R: Rng>(&mut self, entity: Entity, rng: &mut R) {
        self.cleave(entity, 100, rng);
    }

    fn apply_defend<R: Rng>(&mut self, victim: Entity, rng: &mut R) {
        use player::Defend::*;
        let player = self.components.player.get_mut(victim).unwrap();
        if let Some(defend) = player.defend.pop() {
            match defend {
                Dodge => {
                    if let Some(player_coord) = self.spatial.coord(victim) {
                        if let Some(cell) = self.spatial.get_cell(player_coord) {
                            if let Some(floor) = cell.floor {
                                if self.components.sludge.contains(floor) {
                                    return;
                                }
                            }
                        }
                        let mut directions = CardinalDirection::all().collect::<Vec<_>>();
                        directions.shuffle(rng);
                        let maybe_direction = directions
                            .into_iter()
                            .filter_map(|d| {
                                let coord = player_coord + d.coord();
                                if let Some(cell) = self.spatial.get_cell(coord) {
                                    if cell.character.is_none() {
                                        if let Some(floor) = cell.floor {
                                            if self.components.sludge.contains(floor) {
                                                return None;
                                            }
                                        }
                                        if let Some(feature) = cell.feature {
                                            if !self.components.solid.contains(feature) {
                                                return Some(d);
                                            }
                                        } else {
                                            return Some(d);
                                        }
                                    }
                                }
                                None
                            })
                            .next();
                        if let Some(direction) = maybe_direction {
                            let _ = self.character_walk_in_direction(victim, direction, rng);
                        }
                    }
                }
                Armour(n) => {
                    if n > 1 {
                        let _ = player.defend.push(Armour(n - 1));
                    }
                }
                Teleport => self.teleport(victim, rng),
                Revenge => self.revenge(victim, rng),
                SkipAttack => {
                    let player = self.components.player.get_mut(victim).unwrap();
                    player.attack.pop();
                }
            }
        } else {
            self.character_die(victim, rng);
        }
    }

    fn melee_attack<R: Rng>(&mut self, attacker: Entity, victim: Entity, direction: CardinalDirection, rng: &mut R) {
        if self.components.player.get(attacker).is_some() {
            self.player_melee_attack(attacker, victim, direction, rng);
        } else if self.components.player.get(victim).is_some() {
            self.npc_melee_attack(attacker, victim, rng);
        }
    }

    fn open_door(&mut self, door: Entity) {
        self.components.solid.remove(door);
        self.components.opacity.remove(door);
        self.components.tile.insert(door, Tile::DoorOpen);
    }

    pub fn character_fire_bullet(&mut self, character: Entity, target: Coord) {
        let character_coord = self.spatial.coord(character).unwrap();
        if character_coord == target {
            return;
        }
        self.spawn_bullet(character_coord, target);
        self.spawn_flash(character_coord);
    }

    fn blink<R: Rng>(&mut self, entity: Entity, coord: Coord, rng: &mut R) {
        self.spatial.update_coord(entity, coord).unwrap();
        if self.components.player.contains(entity) {
            self.after_player_move(entity, coord, rng);
        }
    }

    pub fn apply_tech_with_coord<R: Rng>(
        &mut self,
        entity: Entity,
        coord: Coord,
        visibility_grid: &VisibilityGrid,
        rng: &mut R,
    ) -> Result<(), Error> {
        use player::Tech::*;
        let player = self.components.player.get_mut(entity).unwrap();
        if let Some(tech) = player.tech.peek() {
            match tech {
                Blink => {
                    if let Some(spatial_cell) = self.spatial.get_cell(coord) {
                        if spatial_cell.character.is_none() && visibility_grid.is_coord_currently_visible(coord) {
                            let can_blink = if let Some(feature) = spatial_cell.feature {
                                !self.components.solid.contains(feature)
                            } else {
                                true
                            };
                            if can_blink {
                                player.tech.pop();
                                self.blink(entity, coord, rng);
                                Ok(())
                            } else {
                                Err(Error::BlinkToSolidCell)
                            }
                        } else {
                            Err(Error::BlinkToNonVisibleCell)
                        }
                    } else {
                        Err(Error::BlinkToNonVisibleCell)
                    }
                }
                _ => return self.apply_tech(entity, rng),
            }
        } else {
            Err(Error::NoTechToApply)
        }
    }

    fn attract(&mut self, entity: Entity) {
        const RANGE: u32 = 12;
        const ATTRACT_BY: u32 = 4;
        let this_coord = self.spatial.coord(entity).unwrap();
        let mut to_push_back = self
            .components
            .npc
            .entities()
            .filter_map(|entity| {
                if let Some(coord) = self.spatial.coord(entity) {
                    let distance2 = this_coord.distance2(coord);
                    if distance2 < RANGE * RANGE {
                        Some((entity, coord, distance2))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        to_push_back.sort_by_key(|&(_, _, distance2)| distance2);
        for (entity, coord, _) in to_push_back {
            self.realtime_components.movement.insert(
                entity,
                ScheduledRealtimePeriodicState {
                    state: movement::spec::Movement {
                        path: this_coord - coord,
                        repeat: movement::spec::Repeat::Steps(ATTRACT_BY as usize),
                        cardinal_step_duration: Duration::from_millis(32),
                    }
                    .build(),
                    until_next_event: Duration::from_millis(0),
                },
            );
            self.components.realtime.insert(entity, ());
            self.components.blocks_gameplay.insert(entity, ());
            self.components.on_collision.insert(entity, OnCollision::RemoveRealtime);
        }
    }

    fn repel(&mut self, entity: Entity) {
        const RANGE: u32 = 12;
        const PUSH_BACK: u32 = 4;
        let this_coord = self.spatial.coord(entity).unwrap();
        let mut to_push_back = self
            .components
            .npc
            .entities()
            .filter_map(|entity| {
                if let Some(coord) = self.spatial.coord(entity) {
                    let distance2 = this_coord.distance2(coord);
                    if distance2 < RANGE * RANGE {
                        Some((entity, coord, distance2))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        to_push_back.sort_by_key(|&(_, _, distance2)| distance2);
        for (entity, coord, _) in to_push_back {
            self.realtime_components.movement.insert(
                entity,
                ScheduledRealtimePeriodicState {
                    state: movement::spec::Movement {
                        path: coord - this_coord,
                        repeat: movement::spec::Repeat::Steps(PUSH_BACK as usize),
                        cardinal_step_duration: Duration::from_millis(32),
                    }
                    .build(),
                    until_next_event: Duration::from_millis(0),
                },
            );
            self.components.realtime.insert(entity, ());
            self.components.blocks_gameplay.insert(entity, ());
            self.components.on_collision.insert(entity, OnCollision::RemoveRealtime);
        }
    }

    pub fn apply_tech<R: Rng>(&mut self, entity: Entity, rng: &mut R) -> Result<(), Error> {
        use player::Tech::*;
        let player = self.components.player.get_mut(entity).unwrap();
        let mut result = Ok(());
        if let Some(tech) = player.tech.peek() {
            match tech {
                Blink => {
                    log::warn!("attempted to blink without destination coord");
                    result = Err(Error::BlinkWithoutDestination);
                }
                CritNext => {
                    if player.attack.push(player::Attack::Hit(99)).is_err() {
                        result = Err(Error::AttackDeckFull);
                    }
                }
                MissNext => {
                    if player.attack.push(player::Attack::Miss).is_err() {
                        result = Err(Error::AttackDeckFull);
                    }
                }
                TeleportNext => {
                    if player.defend.push(player::Defend::Teleport).is_err() {
                        result = Err(Error::DefendDeckFull);
                    }
                }
                Attract => self.attract(entity),
                Repel => self.repel(entity),
                Skip => {
                    player.attack.pop();
                    if player.defend.pop().is_none() {
                        self.character_die(entity, rng);
                    };
                }
            }
        } else {
            return Err(Error::NoTechToApply);
        }
        if result.is_ok() {
            self.components.player.get_mut(entity).unwrap().tech.pop();
            self.wait(entity, rng);
        }
        result
    }

    pub fn character_fire_shotgun<R: Rng>(&mut self, character: Entity, target: Coord, rng: &mut R) {
        const NUM_BULLETS: usize = 12;
        let character_coord = self.spatial.coord(character).unwrap();
        if character_coord == target {
            return;
        }
        for _ in 0..NUM_BULLETS {
            let offset = vector::Radial {
                angle: vector::Radians::random(rng),
                length: rng.gen_range(0., 3.), // TODO make this depend on the distance
            };
            self.spawn_bullet(character_coord, target + offset.to_cartesian().to_coord_round_nearest());
        }
        self.spawn_flash(character_coord);
    }

    pub fn character_fire_rocket(&mut self, character: Entity, target: Coord) {
        let character_coord = self.spatial.coord(character).unwrap();
        if character_coord == target {
            return;
        }
        self.spawn_rocket(character_coord, target);
    }

    pub fn projectile_stop<R: Rng>(
        &mut self,
        projectile_entity: Entity,
        external_events: &mut Vec<ExternalEvent>,
        rng: &mut R,
    ) {
        if let Some(current_coord) = self.spatial.coord(projectile_entity) {
            if let Some(on_collision) = self.components.on_collision.get(projectile_entity).cloned() {
                match on_collision {
                    OnCollision::Explode(explosion_spec) => {
                        explosion::explode(self, current_coord, explosion_spec, external_events, rng);
                        self.spatial.remove(projectile_entity);
                        self.components.remove_entity(projectile_entity);
                        self.entity_allocator.free(projectile_entity);
                        self.realtime_components.remove_entity(projectile_entity);
                    }
                    OnCollision::Remove => {
                        self.spatial.remove(projectile_entity);
                        self.components.remove_entity(projectile_entity);
                        self.entity_allocator.free(projectile_entity);
                        self.realtime_components.remove_entity(projectile_entity);
                    }
                    OnCollision::RemoveRealtime => {
                        self.realtime_components.remove_entity(projectile_entity);
                        self.components.realtime.remove(projectile_entity);
                        self.components.blocks_gameplay.remove(projectile_entity);
                    }
                }
            }
        }
        self.realtime_components.movement.remove(projectile_entity);
    }

    pub fn projectile_move<R: Rng>(
        &mut self,
        projectile_entity: Entity,
        movement_direction: Direction,
        external_events: &mut Vec<ExternalEvent>,
        rng: &mut R,
    ) {
        if let Some(current_coord) = self.spatial.coord(projectile_entity) {
            let next_coord = current_coord + movement_direction.coord();
            let collides_with = self
                .components
                .collides_with
                .get(projectile_entity)
                .cloned()
                .unwrap_or_default();
            if let Some(&spatial_cell) = self.spatial.get_cell(next_coord) {
                if let Some(character_entity) = spatial_cell.character {
                    if let Some(&projectile_damage) = self.components.projectile_damage.get(projectile_entity) {
                        self.apply_projectile_damage(
                            projectile_entity,
                            projectile_damage,
                            movement_direction,
                            character_entity,
                            rng,
                        );
                    }
                }
                if let Some(entity_in_cell) = spatial_cell.feature.or(spatial_cell.character) {
                    if (collides_with.solid
                        && (self.components.solid.contains(entity_in_cell)
                            || self.components.stairs.contains(entity_in_cell)))
                        || (collides_with.character && self.components.character.contains(entity_in_cell))
                    {
                        self.projectile_stop(projectile_entity, external_events, rng);
                        return;
                    }
                }
                let _ignore_if_occupied = self.spatial.update_coord(projectile_entity, next_coord);
            } else {
                self.projectile_stop(projectile_entity, external_events, rng);
                return;
            }
        } else {
            self.components.remove_entity(projectile_entity);
            self.realtime_components.remove_entity(projectile_entity);
            self.spatial.remove(projectile_entity);
        }
    }

    fn nearest_spawn_candidate<R: Rng>(spatial: &Spatial, start: Coord, rng: &mut R) -> Option<Coord> {
        if let Some(cell) = spatial.get_cell(start) {
            if cell.feature.is_none() {
                if cell.character.is_none() {
                    return Some(start);
                }
            }
        }
        let mut queue = VecDeque::new();
        let mut seen = HashSet::new();
        let mut directions = CardinalDirection::all().collect::<Vec<_>>();
        queue.push_front(start);
        seen.insert(start);
        while let Some(coord) = queue.pop_back() {
            directions.shuffle(rng);
            for &direction in directions.iter() {
                let neighbour_coord = coord + direction.coord();
                if seen.insert(neighbour_coord) {
                    if let Some(cell) = spatial.get_cell(neighbour_coord) {
                        if cell.feature.is_none() {
                            if cell.character.is_none() {
                                return Some(neighbour_coord);
                            }
                            queue.push_front(neighbour_coord);
                        }
                    }
                }
            }
        }
        None
    }

    fn divide<R: Rng>(&mut self, entity: Entity, rng: &mut R) {
        if let Some(coord) = self.spatial.coord(entity) {
            if let Some(hit_points) = self.components.hit_points.get_mut(entity) {
                let new_hit_points = {
                    let mut hit_points = *hit_points;
                    hit_points.current /= 2;
                    hit_points
                };
                if new_hit_points.current > 0 {
                    *hit_points = new_hit_points;
                    let spawn_coord = Self::nearest_spawn_candidate(&self.spatial, coord, rng);
                    if let Some(spawn_coord) = spawn_coord {
                        let mut new_entity_data = self.components.clone_entity_data(entity);
                        new_entity_data.next_action = None;
                        self.insert_entity_data(
                            Location {
                                coord: spawn_coord,
                                layer: Some(Layer::Character),
                            },
                            new_entity_data,
                        );
                    }
                }
            }
        }
    }

    fn divide_and_spawn<R: Rng>(&mut self, entity: Entity, rng: &mut R) {
        self.divide(entity, rng);
        if let Some(coord) = self.spatial.coord(entity) {
            if let Some(spawn_coord) = Self::nearest_spawn_candidate(&self.spatial, coord, rng) {
                match rng.gen_range(0, 3) {
                    0 => {
                        self.spawn_slime_goo(spawn_coord, rng);
                    }
                    1 => {
                        self.spawn_slime_divide(spawn_coord, rng);
                    }
                    2 => {
                        self.spawn_slime_teleport(spawn_coord, rng);
                    }
                    _ => (),
                }
            }
        }
    }

    pub fn damage_character<R: Rng>(&mut self, character: Entity, hit_points_to_lose: u32, rng: &mut R) {
        if let Some(hit_points) = self.components.hit_points.get_mut(character) {
            let coord = self.spatial.coord(character).unwrap();
            let dies = match hit_points.current.checked_sub(hit_points_to_lose) {
                None | Some(0) => {
                    hit_points.current = 0;
                    true
                }
                Some(non_zero_remaining_hit_points) => {
                    hit_points.current = non_zero_remaining_hit_points;
                    false
                }
            };
            if let Some(on_damage) = self.components.on_damage.get(character) {
                match on_damage {
                    OnDamage::Sludge => {
                        if let Some(coord) = self.spatial.coord(character) {
                            self.change_floor_to_sludge(coord);
                        }
                    }
                    OnDamage::Divide => self.divide(character, rng),
                    OnDamage::DivideAndSpawn => self.divide_and_spawn(character, rng),
                    OnDamage::Teleport => {
                        let maybe_player_entity = self.components.player.entities().next();
                        if let Some(player_entity) = maybe_player_entity {
                            if let Some(player_coord) = self.spatial.coord(player_entity) {
                                if let Some(victim_coord) = self.spatial.coord(character) {
                                    if player_coord.manhattan_distance(victim_coord) == 1 {
                                        self.teleport(player_entity, rng);
                                    }
                                }
                            }
                        }
                        self.teleport(character, rng);
                    }
                    OnDamage::Swap => {
                        let maybe_player_entity = self.components.player.entities().next();
                        if let Some(player_entity) = maybe_player_entity {
                            if let Some(player_coord) = self.spatial.coord(player_entity) {
                                if let Some(victim_coord) = self.spatial.coord(character) {
                                    if player_coord.manhattan_distance(victim_coord) == 1 {
                                        self.spatial.remove(player_entity);
                                        self.spatial.update_coord(character, player_coord).unwrap();
                                        self.spatial
                                            .insert(
                                                player_entity,
                                                Location {
                                                    coord: victim_coord,
                                                    layer: Some(Layer::Character),
                                                },
                                            )
                                            .unwrap();
                                    }
                                }
                            }
                        }
                    }
                    OnDamage::Upgrade { level, ability_target } => {
                        let maybe_player_entity = self.components.player.entities().next();
                        if let Some(player_entity) = maybe_player_entity {
                            let player = self.components.player.get_mut(player_entity).unwrap();
                            use player::AbilityTarget::*;
                            match ability_target {
                                Attack => {
                                    let _ = player
                                        .attack
                                        .insert_random(player::choose_attack_upgrade(*level, rng), rng);
                                    let _ = player
                                        .attack
                                        .insert_random(player::choose_attack_upgrade(*level, rng), rng);
                                }
                                Defend => {
                                    let _ = player
                                        .defend
                                        .insert_random(player::choose_defend_upgrade(*level, rng), rng);
                                }
                                Tech => {
                                    let _ = player.tech.insert_random(player::choose_tech_upgrade(*level, rng), rng);
                                }
                            }
                        }
                    }
                    OnDamage::Curse => {
                        let maybe_player_entity = self.components.player.entities().next();
                        if let Some(player_entity) = maybe_player_entity {
                            let player = self.components.player.get_mut(player_entity).unwrap();
                            use player::Outcome;
                            let _ = match player::choose_curse(rng) {
                                Outcome::Attack(attack) => player.attack.insert_random(attack, rng),
                                Outcome::Defend(defend) => player.defend.insert_random(defend, rng),
                                Outcome::Tech(tech) => player.tech.insert_random(tech, rng),
                            };
                        }
                    }
                }
            }
            self.add_blood_stain_to_floor(coord);
            if dies {
                self.character_die(character, rng);
            }
        } else {
            log::warn!("attempt to damage entity without hit_points component");
        }
    }

    fn character_push_in_direction(&mut self, entity: Entity, direction: Direction) {
        if let Some(current_coord) = self.spatial.coord(entity) {
            let target_coord = current_coord + direction.coord();
            if self.is_solid_feature_at_coord(target_coord) {
                return;
            }
            let _ignore_if_occupied = self.spatial.update_coord(entity, target_coord);
        }
    }

    fn change_floor_to_sludge(&mut self, coord: Coord) {
        if let Some(&cell) = self.spatial.get_cell(coord) {
            if let Some(floor_entity) = cell.floor {
                self.spatial.remove(floor_entity);
                self.components.remove_entity(floor_entity);
                self.realtime_components.remove_entity(floor_entity);
            }
            if let Some(feature_entity) = cell.feature {
                if self.components.door_state.contains(feature_entity) {
                    self.spatial.remove(feature_entity);
                    self.components.remove_entity(feature_entity);
                    self.realtime_components.remove_entity(feature_entity);
                }
            }
        }
        self.spawn_sludge(coord);
        self.spawn_sludge_light(coord);
    }

    fn character_die<R: Rng>(&mut self, character: Entity, rng: &mut R) {
        self.components.to_remove.insert(character, ());
        if let Some(drop_item_on_death) = self.components.drop_item_on_death.get(character) {
            if let Some(coord) = self.spatial.coord(character) {
                if let Some(cell) = self.spatial.get_cell(coord) {
                    let spawn_coord = if cell.feature.is_none() {
                        Some(coord)
                    } else {
                        let mut queue = VecDeque::new();
                        let mut seen = HashSet::new();
                        let mut directions = CardinalDirection::all().collect::<Vec<_>>();
                        let mut spawn_coord = None;
                        queue.push_front(coord);
                        seen.insert(coord);
                        while let Some(coord) = queue.pop_back() {
                            directions.shuffle(rng);
                            for &direction in directions.iter() {
                                let neighbour_coord = coord + direction.coord();
                                if seen.insert(neighbour_coord) {
                                    if let Some(cell) = self.spatial.get_cell(neighbour_coord) {
                                        if let Some(feature) = cell.feature {
                                            if !self.components.solid.contains(feature) {
                                                queue.push_front(neighbour_coord);
                                            }
                                        } else {
                                            spawn_coord = Some(neighbour_coord);
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        spawn_coord
                    };
                    if let Some(spawn_coord) = spawn_coord {
                        match drop_item_on_death {
                            DropItemOnDeath::GuaranteeSpecial => match rng.gen_range(0, 5) {
                                0 => {
                                    self.spawn_defend(spawn_coord, true);
                                }
                                1 => {
                                    self.spawn_tech(spawn_coord, true);
                                }
                                2..=4 => {
                                    self.spawn_attack(spawn_coord, true);
                                }
                                _ => unreachable!(),
                            },
                            DropItemOnDeath::RandomNormal => match rng.gen_range(0, 2) {
                                0 => match rng.gen_range(0, 5) {
                                    0 => {
                                        self.spawn_defend(spawn_coord, false);
                                    }
                                    1 => {
                                        self.spawn_tech(spawn_coord, false);
                                    }
                                    2..=4 => {
                                        self.spawn_attack(spawn_coord, false);
                                    }
                                    _ => unreachable!(),
                                },
                                1 => (),
                                _ => unreachable!(),
                            },
                        }
                    }
                }
            }
        }
    }

    fn add_blood_stain_to_floor(&mut self, coord: Coord) {
        if let Some(floor_entity) = self.spatial.get_cell_checked(coord).floor {
            self.components.blood.insert(floor_entity, ());
        }
    }

    fn apply_projectile_damage<R: Rng>(
        &mut self,
        projectile_entity: Entity,
        projectile_damage: ProjectileDamage,
        projectile_movement_direction: Direction,
        entity_to_damage: Entity,
        rng: &mut R,
    ) {
        self.damage_character(entity_to_damage, projectile_damage.hit_points, rng);
        if projectile_damage.push_back {
            self.character_push_in_direction(entity_to_damage, projectile_movement_direction);
        }
        self.components.remove_entity(projectile_entity);
    }

    pub fn sludge_damage<R: Rng>(&mut self, rng: &mut R) {
        const DAMAGE: u32 = 4;
        for entity in self
            .components
            .npc
            .entities()
            .filter(|&entity| {
                if self.components.safe_on_sludge.contains(entity) {
                    return false;
                }
                if let Some(coord) = self.spatial.coord(entity) {
                    if let Some(cell) = self.spatial.get_cell(coord) {
                        if let Some(floor) = cell.floor {
                            if self.components.sludge.contains(floor) {
                                return true;
                            }
                        }
                    }
                }
                false
            })
            .collect::<Vec<_>>()
        {
            self.damage_character(entity, DAMAGE, rng);
        }
    }
}
