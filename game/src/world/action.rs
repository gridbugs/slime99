use crate::{
    world::{
        data::{DoorState, OnCollision, ProjectileDamage, Tile},
        explosion, player,
        spatial::OccupiedBy,
        ExternalEvent, World,
    },
    VisibilityGrid,
};
use direction::{CardinalDirection, Direction};
use ecs::Entity;
use grid_2d::Coord;
use rand::Rng;

impl World {
    pub fn character_walk_in_direction(&mut self, character: Entity, direction: CardinalDirection) {
        let &current_coord = if let Some(coord) = self.spatial.coord(character) {
            coord
        } else {
            panic!("failed to find coord for {:?}", character);
        };
        let target_coord = current_coord + direction.coord();
        if let Some(feature_entity) = self.spatial.get_cell(target_coord).and_then(|cell| cell.feature) {
            if self.components.solid.contains(feature_entity) {
                if let Some(DoorState::Closed) = self.components.door_state.get(feature_entity).cloned() {
                    self.open_door(feature_entity);
                }
                return;
            }
        }
        if let Err(OccupiedBy(occupant)) = self.spatial.update_coord(character, target_coord) {
            self.melee_attack(character, occupant);
        }
    }

    pub fn player_melee_attack(&mut self, attacker: Entity, victim: Entity) {
        let player = self.components.player.get_mut(attacker).unwrap();
        if let Some(attack) = player.attack.pop() {
            self.apply_attack(attack, attacker, victim);
        }
    }

    pub fn npc_melee_attack(&mut self, attacker: Entity, victim: Entity) {
        let player = self.components.player.get_mut(victim).unwrap();
        if let Some(defend) = player.defend.pop() {
            self.apply_defend(defend, attacker, victim);
        } else {
            self.character_die(victim);
        }
    }

    fn cleave(&mut self, entity: Entity, damage: u32) {}
    fn skewer(&mut self, entity: Entity, damage: u32) {}

    pub fn apply_attack(&mut self, attack: player::Attack, attacker: Entity, victim: Entity) {
        use player::Attack::*;
        match attack {
            Miss => (),
            Hit(n) => self.damage_character(victim, n),
            Cleave(n) => self.cleave(attacker, n),
            Skewer(n) => self.skewer(attacker, n),
        }
    }

    fn teleport(&mut self, entity: Entity) {}
    fn panic(&mut self, entity: Entity) {}

    pub fn apply_defend(&mut self, defend: player::Defend, _attacker: Entity, victim: Entity) {
        use player::Defend::*;
        match defend {
            Dodge => (),
            Teleport => self.teleport(victim),
            Panic => self.panic(victim),
        }
    }

    pub fn melee_attack(&mut self, attacker: Entity, victim: Entity) {
        if self.components.player.get(attacker).is_some() {
            self.player_melee_attack(attacker, victim);
        } else if self.components.player.get(victim).is_some() {
            self.npc_melee_attack(attacker, victim);
        }
    }

    pub fn open_door(&mut self, door: Entity) {
        self.components.solid.remove(door);
        self.components.opacity.remove(door);
        self.components.tile.insert(door, Tile::DoorOpen);
    }

    pub fn character_fire_bullet(&mut self, character: Entity, target: Coord) {
        let &character_coord = self.spatial.coord(character).unwrap();
        if character_coord == target {
            return;
        }
        self.spawn_bullet(character_coord, target);
        self.spawn_flash(character_coord);
    }

    fn blink(&mut self, entity: Entity, coord: Coord) {
        self.spatial.update_coord(entity, coord).unwrap();
    }

    pub fn apply_tech_with_coord(&mut self, entity: Entity, coord: Coord, visibility_grid: &VisibilityGrid) {
        use player::Tech::*;
        let player = self.components.player.get_mut(entity).unwrap();
        if let Some(tech) = player.tech.peek() {
            match tech {
                Blink => {
                    if let Some(spatial_cell) = self.spatial.get_cell(coord) {
                        if spatial_cell.character.is_none() && visibility_grid.is_coord_visible(coord) {
                            let can_blink = if let Some(feature) = spatial_cell.feature {
                                !self.components.solid.contains(feature)
                            } else {
                                true
                            };
                            if can_blink {
                                player.tech.pop();
                                self.blink(entity, coord);
                            }
                        }
                    }
                }
                _ => self.apply_tech(entity),
            }
        }
    }

    fn attract(&mut self, entity: Entity) {}
    fn repel(&mut self, entity: Entity) {}

    pub fn apply_tech(&mut self, entity: Entity) {
        use player::Tech::*;
        let player = self.components.player.get_mut(entity).unwrap();
        let mut success = true;
        if let Some(tech) = player.tech.peek() {
            match tech {
                Blink => {
                    log::warn!("attempted to blink without destination coord");
                    success = false;
                }
                CritNext => {
                    if player.attack.push(player::Attack::Hit(99)).is_err() {
                        success = false;
                    }
                }
                MissNext => {
                    if player.attack.push(player::Attack::Miss).is_err() {
                        success = false;
                    }
                }
                TeleportNext => {
                    if player.defend.push(player::Defend::Teleport).is_err() {
                        success = false;
                    }
                }
                Attract => self.attract(entity),
                Repel => self.repel(entity),
            }
        }
        if success {
            self.components.player.get_mut(entity).unwrap().tech.pop();
        }
    }

    pub fn character_fire_shotgun<R: Rng>(&mut self, character: Entity, target: Coord, rng: &mut R) {
        const NUM_BULLETS: usize = 12;
        let &character_coord = self.spatial.coord(character).unwrap();
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
        let &character_coord = self.spatial.coord(character).unwrap();
        if character_coord == target {
            return;
        }
        self.spawn_rocket(character_coord, target);
    }

    pub fn projectile_stop(&mut self, projectile_entity: Entity, external_events: &mut Vec<ExternalEvent>) {
        if let Some(&current_coord) = self.spatial.coord(projectile_entity) {
            if let Some(on_collision) = self.components.on_collision.get(projectile_entity).cloned() {
                match on_collision {
                    OnCollision::Explode(explosion_spec) => {
                        explosion::explode(self, current_coord, explosion_spec, external_events);
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
                }
            }
        }
        self.realtime_components.movement.remove(projectile_entity);
    }

    pub fn projectile_move(
        &mut self,
        projectile_entity: Entity,
        movement_direction: Direction,
        external_events: &mut Vec<ExternalEvent>,
    ) {
        if let Some(&current_coord) = self.spatial.coord(projectile_entity) {
            let next_coord = current_coord + movement_direction.coord();
            let collides_with = self
                .components
                .collides_with
                .get(projectile_entity)
                .cloned()
                .unwrap_or_default();
            let &spatial_cell = self.spatial.get_cell_checked(next_coord);
            if let Some(character_entity) = spatial_cell.character {
                if let Some(&projectile_damage) = self.components.projectile_damage.get(projectile_entity) {
                    self.apply_projectile_damage(
                        projectile_entity,
                        projectile_damage,
                        movement_direction,
                        character_entity,
                    );
                }
            }
            if let Some(entity_in_cell) = spatial_cell.feature.or(spatial_cell.character) {
                if (collides_with.solid && self.components.solid.contains(entity_in_cell))
                    || (collides_with.character && self.components.character.contains(entity_in_cell))
                {
                    self.projectile_stop(projectile_entity, external_events);
                    return;
                }
            }
            let _ignore_if_occupied = self.spatial.update_coord(projectile_entity, next_coord);
        } else {
            self.components.remove_entity(projectile_entity);
            self.realtime_components.remove_entity(projectile_entity);
            self.spatial.remove(projectile_entity);
        }
    }

    pub fn damage_character(&mut self, character: Entity, hit_points_to_lose: u32) {
        if let Some(hit_points) = self.components.hit_points.get_mut(character) {
            let &coord = self.spatial.coord(character).unwrap();
            match hit_points.current.checked_sub(hit_points_to_lose) {
                None | Some(0) => {
                    hit_points.current = 0;
                    self.character_die(character);
                }
                Some(non_zero_remaining_hit_points) => {
                    hit_points.current = non_zero_remaining_hit_points;
                }
            }
            self.add_blood_stain_to_floor(coord);
        } else {
            log::warn!("attempt to damage entity without hit_points component");
        }
    }

    fn character_push_in_direction(&mut self, entity: Entity, direction: Direction) {
        if let Some(&current_coord) = self.spatial.coord(entity) {
            let target_coord = current_coord + direction.coord();
            if self.is_solid_feature_at_coord(target_coord) {
                return;
            }
            let _ignore_if_occupied = self.spatial.update_coord(entity, target_coord);
        }
    }

    fn character_die(&mut self, character: Entity) {
        self.components.to_remove.insert(character, ());
    }

    fn add_blood_stain_to_floor(&mut self, coord: Coord) {
        if let Some(floor_entity) = self.spatial.get_cell_checked(coord).floor {
            self.components.blood.insert(floor_entity, ());
        }
    }

    fn apply_projectile_damage(
        &mut self,
        projectile_entity: Entity,
        projectile_damage: ProjectileDamage,
        projectile_movement_direction: Direction,
        entity_to_damage: Entity,
    ) {
        self.damage_character(entity_to_damage, projectile_damage.hit_points);
        if projectile_damage.push_back {
            self.character_push_in_direction(entity_to_damage, projectile_movement_direction);
        }
        self.components.remove_entity(projectile_entity);
    }
}
