pub use direction::CardinalDirection;
pub use grid_2d::{Coord, Grid, Size};
use rand::{Rng, SeedableRng};
use rand_isaac::Isaac64Rng;
use serde::{Deserialize, Serialize};
use shadowcast::Context as ShadowcastContext;
use std::time::Duration;

mod behaviour;
mod terrain;
mod visibility;
mod world;

use behaviour::{Agent, BehaviourContext};
use ecs::ComponentTable;
pub use ecs::Entity;
use procgen::SewerSpec;
use terrain::Terrain;
pub use visibility::{CellVisibility, Omniscient, VisibilityGrid};
use world::{make_player, AnimationContext, World, ANIMATION_FRAME_DURATION};
pub use world::{player, CharacterInfo, EntityData, HitPoints, Layer, NpcAction, PlayerDied, Tile, ToRenderEntity};

pub const MAP_SIZE: Size = Size::new_u16(19, 19);

pub struct Config {
    pub omniscient: Option<Omniscient>,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum Music {
    Fiberitron,
}

/// Events which the game can report back to the io layer so it can
/// respond with a sound/visual effect.
#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum ExternalEvent {
    Explosion(Coord),
    LoopMusic(Music),
}

pub enum GameControlFlow {
    GameOver,
}

#[derive(Clone, Copy, Debug)]
pub enum Input {
    Walk(CardinalDirection),
    Fire(Coord),
    Wait,
}

#[derive(Serialize, Deserialize)]
pub struct Game {
    world: World,
    visibility_grid: VisibilityGrid,
    player: Entity,
    last_player_info: CharacterInfo,
    rng: Isaac64Rng,
    frame_count: u64,
    events: Vec<ExternalEvent>,
    shadowcast_context: ShadowcastContext<u8>,
    behaviour_context: BehaviourContext,
    animation_context: AnimationContext,
    agents: ComponentTable<Agent>,
    agents_to_remove: Vec<Entity>,
    since_last_frame: Duration,
    generate_frame_countdown: Option<Duration>,
    dead_player: Option<EntityData>,
}

impl Game {
    pub fn new<R: Rng>(config: &Config, rng: &mut R) -> Self {
        let mut rng = Isaac64Rng::seed_from_u64(rng.gen());
        //let Terrain { world, agents, player } = terrain::from_str(include_str!("terrain.txt"), make_player());
        let Terrain { world, agents, player } = terrain::sewer(SewerSpec { size: MAP_SIZE }, make_player(), &mut rng);
        let last_player_info = world.character_info(player).expect("couldn't get info for player");
        let events = vec![ExternalEvent::LoopMusic(Music::Fiberitron)];
        let mut game = Self {
            visibility_grid: VisibilityGrid::new(world.size()),
            player,
            last_player_info,
            rng,
            frame_count: 0,
            events,
            shadowcast_context: ShadowcastContext::default(),
            behaviour_context: BehaviourContext::new(world.size()),
            animation_context: AnimationContext::default(),
            agents,
            agents_to_remove: Vec::new(),
            world,
            since_last_frame: Duration::from_millis(0),
            generate_frame_countdown: None,
            dead_player: None,
        };
        game.update_behaviour();
        game.update_visibility(config);
        game.prime_npcs();
        game
    }
    pub fn size(&self) -> Size {
        self.world.size()
    }
    fn cleanup(&mut self) {
        if let Some(PlayerDied(player_data)) = self.world.cleanup() {
            self.dead_player = Some(player_data);
        }
    }
    pub fn is_gameplay_blocked(&self) -> bool {
        self.world.is_gameplay_blocked()
    }
    pub fn update_visibility(&mut self, config: &Config) {
        if let Some(player_coord) = self.world.entity_coord(self.player) {
            self.visibility_grid.update(
                player_coord,
                &self.world,
                &mut self.shadowcast_context,
                config.omniscient,
            );
        }
    }
    fn update_behaviour(&mut self) {
        self.behaviour_context.update(self.player, &self.world);
    }
    #[must_use]
    pub fn handle_input(&mut self, input: Input, config: &Config) -> Option<GameControlFlow> {
        if self.generate_frame_countdown.is_some() {
            return None;
        }
        if !self.is_gameplay_blocked() {
            match input {
                Input::Walk(direction) => self.world.character_walk_in_direction(self.player, direction),
                Input::Fire(coord) => self.world.character_fire_rocket(self.player, coord),
                Input::Wait => (),
            }
        }
        self.cleanup();
        self.update_visibility(config);
        self.update_behaviour();
        self.npc_turn();
        self.update_last_player_info();
        if self.is_game_over() {
            Some(GameControlFlow::GameOver)
        } else {
            None
        }
    }
    pub fn handle_npc_turn(&mut self) {
        if !self.is_gameplay_blocked() {
            self.update_behaviour();
            self.npc_turn();
        }
    }
    fn prime_npcs(&mut self) {
        for (entity, agent) in self.agents.iter_mut() {
            let next_action = agent.act(
                entity,
                &self.world,
                self.player,
                &mut self.behaviour_context,
                &mut self.shadowcast_context,
                &mut self.rng,
            );
            self.world.commit_to_next_action(entity, next_action);
        }
    }
    fn npc_turn(&mut self) {
        for (entity, agent) in self.agents.iter_mut() {
            if !self.world.entity_exists(entity) {
                self.agents_to_remove.push(entity);
                continue;
            }
            let current_action = self.world.next_npc_action(entity).unwrap_or(NpcAction::Wait);
            match current_action {
                NpcAction::Wait => (),
                NpcAction::Walk(direction) => self.world.character_walk_in_direction(entity, direction),
            }
            let next_action = agent.act(
                entity,
                &self.world,
                self.player,
                &mut self.behaviour_context,
                &mut self.shadowcast_context,
                &mut self.rng,
            );
            self.world.commit_to_next_action(entity, next_action);
        }
        for entity in self.agents_to_remove.drain(..) {
            self.agents.remove(entity);
        }
        self.cleanup();
        self.after_turn();
    }
    fn generate_level(&mut self, config: &Config) {
        let player_data = self.world.clone_entity_data(self.player);
        let Terrain { world, agents, player } =
            terrain::sewer(SewerSpec { size: MAP_SIZE }, player_data, &mut self.rng);
        self.visibility_grid = VisibilityGrid::new(world.size());
        self.world = world;
        self.agents = agents;
        self.player = player;
        self.update_last_player_info();
        self.update_behaviour();
        self.update_visibility(config);
        self.prime_npcs();
        self.events.push(ExternalEvent::LoopMusic(Music::Fiberitron));
    }
    fn after_turn(&mut self) {
        if let Some(player_coord) = self.world.entity_coord(self.player) {
            if let Some(_stairs_entity) = self.world.get_stairs_at_coord(player_coord) {
                self.generate_frame_countdown = Some(Duration::from_millis(200));
            }
        }
    }
    pub fn is_generating(&self) -> bool {
        if let Some(countdown) = self.generate_frame_countdown {
            countdown.as_millis() == 0
        } else {
            false
        }
    }
    #[must_use]
    pub fn handle_tick(&mut self, since_last_tick: Duration, config: &Config) -> Option<GameControlFlow> {
        if let Some(countdown) = self.generate_frame_countdown.as_mut() {
            if countdown.as_millis() == 0 {
                self.generate_level(config);
                self.generate_frame_countdown = None;
            } else {
                *countdown = if let Some(remaining) = countdown.checked_sub(since_last_tick) {
                    remaining
                } else {
                    Duration::from_millis(0)
                };
            }
            return None;
        }
        self.since_last_frame += since_last_tick;
        while let Some(remaining_since_last_frame) = self.since_last_frame.checked_sub(ANIMATION_FRAME_DURATION) {
            self.since_last_frame = remaining_since_last_frame;
            if let Some(game_control_flow) = self.handle_tick_inner(config) {
                return Some(game_control_flow);
            }
        }
        None
    }
    fn handle_tick_inner(&mut self, config: &Config) -> Option<GameControlFlow> {
        self.update_visibility(config);
        self.world
            .animation_tick(&mut self.animation_context, &mut self.events, &mut self.rng);
        self.frame_count += 1;
        self.update_last_player_info();
        if self.is_game_over() {
            Some(GameControlFlow::GameOver)
        } else {
            None
        }
    }
    pub fn events(&mut self) -> impl '_ + Iterator<Item = ExternalEvent> {
        self.events.drain(..)
    }
    pub fn player_info(&self) -> &CharacterInfo {
        &self.last_player_info
    }
    pub fn world_size(&self) -> Size {
        self.world.size()
    }
    pub fn to_render_entities<'a>(&'a self) -> impl 'a + Iterator<Item = ToRenderEntity> {
        self.world.to_render_entities()
    }
    pub fn visibility_grid(&self) -> &VisibilityGrid {
        &self.visibility_grid
    }
    pub fn contains_wall(&self, coord: Coord) -> bool {
        self.world.is_wall_at_coord(coord)
    }
    pub fn contains_bridge(&self, coord: Coord) -> bool {
        self.world.is_bridge_at_coord(coord)
    }
    fn update_last_player_info(&mut self) {
        if let Some(character_info) = self.world.character_info(self.player) {
            self.last_player_info = character_info;
        }
    }
    fn is_game_over(&self) -> bool {
        self.dead_player.is_some()
    }
    pub fn player(&self) -> &player::Player {
        if let Some(player) = self.world.entity_player(self.player) {
            player
        } else {
            self.dead_player.as_ref().unwrap().player.as_ref().unwrap()
        }
    }
}
