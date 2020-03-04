use crate::audio::{Audio, AudioTable};
use crate::controls::{AppInput, Controls};
use crate::frontend::Frontend;
use crate::render::{GameToRender, GameView, Mode};
use direction::{CardinalDirection, Direction};
use game::{ActionError, CharacterInfo, ExternalEvent, Game, GameControlFlow, Music};
pub use game::{Config as GameConfig, Input as GameInput, Omniscient};
use prototty::event_routine::common_event::*;
use prototty::event_routine::*;
use prototty::input::*;
use prototty_audio::{AudioHandle, AudioPlayer};
use prototty_storage::{format, Storage};
use rand::{Rng, SeedableRng};
use rand_isaac::Isaac64Rng;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::time::Duration;

const CONFIG_KEY: &str = "config.json";

const GAME_MUSIC_VOLUME: f32 = 0.05;
const MENU_MUSIC_VOLUME: f32 = 0.02;

const PLAYER_OFFSET: Coord = Coord::new(30, 18);
const STORAGE_FORMAT: format::Bincode = format::Bincode;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Config {
    pub music: bool,
    pub sfx: bool,
    pub fullscreen: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            music: true,
            sfx: true,
            fullscreen: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy)]
struct ScreenShake {
    remaining_frames: u8,
    direction: Direction,
}

impl ScreenShake {
    fn _coord(&self) -> Coord {
        self.direction.coord()
    }
    fn next(self) -> Option<Self> {
        self.remaining_frames.checked_sub(1).map(|remaining_frames| Self {
            remaining_frames,
            direction: self.direction,
        })
    }
}

struct EffectContext<'a, A: AudioPlayer> {
    rng: &'a mut Isaac64Rng,
    screen_shake: &'a mut Option<ScreenShake>,
    current_music: &'a mut Option<Music>,
    current_music_handle: &'a mut Option<A::Handle>,
    audio_player: &'a A,
    audio_table: &'a AudioTable<A>,
    player_coord: GameCoord,
    config: &'a Config,
}

impl<'a, A: AudioPlayer> EffectContext<'a, A> {
    fn next_frame(&mut self) {
        *self.screen_shake = self.screen_shake.and_then(|screen_shake| screen_shake.next());
    }
    fn play_audio(&self, audio: Audio, volume: f32) {
        log::info!("Playing audio {:?} at volume {:?}", audio, volume);
        let sound = self.audio_table.get(audio);
        let handle = self.audio_player.play(&sound);
        handle.set_volume(volume);
        handle.background();
    }
    fn handle_event(&mut self, event: ExternalEvent) {
        match event {
            ExternalEvent::Explosion(coord) => {
                let direction: Direction = self.rng.gen();
                *self.screen_shake = Some(ScreenShake {
                    remaining_frames: 2,
                    direction,
                });
                if self.config.sfx {
                    const BASE_VOLUME: f32 = 50.;
                    let distance_squared = (self.player_coord.0 - coord).magnitude2();
                    let volume = (BASE_VOLUME / (distance_squared as f32).max(1.)).min(1.);
                    self.play_audio(Audio::Explosion, volume);
                }
            }
            ExternalEvent::LoopMusic(music) => {
                *self.current_music = Some(music);
                let handle = loop_music(self.audio_player, self.audio_table, self.config, music);
                *self.current_music_handle = Some(handle);
            }
        }
    }
}

fn loop_music<A: AudioPlayer>(
    audio_player: &A,
    audio_table: &AudioTable<A>,
    config: &Config,
    music: Music,
) -> A::Handle {
    let audio = match music {
        Music::Fiberitron => Audio::Fiberitron,
    };
    let volume = GAME_MUSIC_VOLUME;
    log::info!("Looping audio {:?} at volume {:?}", audio, volume);
    let sound = audio_table.get(audio);
    let handle = audio_player.play_loop(&sound);
    handle.set_volume(volume);
    if !config.music {
        handle.pause();
    }
    handle
}

pub enum InjectedInput {
    Tech(Coord),
}

#[derive(Clone, Copy)]
pub struct ScreenCoord(pub Coord);

#[derive(Clone, Copy)]
struct GameCoord(Coord);

#[derive(Clone, Copy)]
struct PlayerCoord(Coord);

impl GameCoord {
    fn of_player(player_info: &CharacterInfo) -> Self {
        Self(player_info.coord)
    }
}

struct GameCoordToScreenCoord {
    game_coord: GameCoord,
    player_coord: GameCoord,
}

impl GameCoordToScreenCoord {
    fn compute(self) -> ScreenCoord {
        ScreenCoord(self.game_coord.0 - self.player_coord.0 + PLAYER_OFFSET)
    }
}

#[derive(Serialize, Deserialize)]
pub struct GameInstance {
    rng: Isaac64Rng,
    game: Game,
    screen_shake: Option<ScreenShake>,
    current_music: Option<Music>,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum GameStatus {
    Playing,
    Over,
}

#[derive(Clone, Copy, Debug)]
pub enum RngSeed {
    Random,
    U64(u64),
}

impl GameInstance {
    fn new(game_config: &GameConfig, mut rng: Isaac64Rng) -> Self {
        Self {
            game: Game::new(game_config, &mut rng),
            rng,
            screen_shake: None,
            current_music: None,
        }
    }
    pub fn game(&self) -> &Game {
        &self.game
    }
}

pub struct GameData<S: Storage, A: AudioPlayer> {
    instance: Option<GameInstance>,
    controls: Controls,
    rng_seed_source: RngSeedSource,
    last_aim_with_mouse: bool,
    storage_wrapper: StorageWrapper<S>,
    audio_player: A,
    audio_table: AudioTable<A>,
    game_config: GameConfig,
    frontend: Frontend,
    music_handle: Option<A::Handle>,
    config: Config,
}

struct StorageWrapper<S: Storage> {
    storage: S,
    save_key: String,
}

impl<S: Storage> StorageWrapper<S> {
    pub fn save_instance(&mut self, instance: &GameInstance) {
        self.storage
            .store(&self.save_key, instance, STORAGE_FORMAT)
            .expect("failed to save instance");
    }
    pub fn clear_instance(&mut self) {
        let _ = self.storage.remove(&self.save_key);
    }
}

struct RngSeedSource {
    rng: Isaac64Rng,
    next: u64,
}

impl RngSeedSource {
    fn new(rng_seed: RngSeed) -> Self {
        let mut rng = Isaac64Rng::from_entropy();
        let next = match rng_seed {
            RngSeed::Random => rng.gen(),
            RngSeed::U64(seed) => seed,
        };
        Self { rng, next }
    }
    fn next_seed(&mut self) -> u64 {
        let seed = self.next;
        self.next = self.rng.gen();
        seed
    }
}

impl<S: Storage, A: AudioPlayer> GameData<S, A> {
    pub fn new(
        game_config: GameConfig,
        controls: Controls,
        storage: S,
        save_key: String,
        audio_player: A,
        rng_seed: RngSeed,
        frontend: Frontend,
    ) -> Self {
        let config = storage.load(CONFIG_KEY, format::Json).unwrap_or_default();
        let mut instance: Option<GameInstance> = match storage.load(&save_key, STORAGE_FORMAT) {
            Ok(instance) => Some(instance),
            Err(e) => {
                log::info!("no instance found: {:?}", e);
                None
            }
        };
        if let Some(instance) = instance.as_mut() {
            instance.game.update_visibility(&game_config);
        }
        let rng_seed_source = RngSeedSource::new(rng_seed);
        let storage_wrapper = StorageWrapper { storage, save_key };
        let audio_table = AudioTable::new(&audio_player);
        let music_handle = if let Some(instance) = instance.as_ref() {
            if let Some(music) = instance.current_music {
                let handle = loop_music(&audio_player, &audio_table, &config, music);
                Some(handle)
            } else {
                None
            }
        } else {
            None
        };
        Self {
            instance,
            controls,
            rng_seed_source,
            last_aim_with_mouse: false,
            storage_wrapper,
            audio_table,
            audio_player,
            game_config,
            frontend,
            music_handle,
            config,
        }
    }
    pub fn config(&self) -> Config {
        self.config
    }
    pub fn set_config(&mut self, config: Config) {
        self.config = config;
        if let Some(music_handle) = self.music_handle.as_ref() {
            if config.music {
                music_handle.play();
            } else {
                music_handle.pause();
            }
        }
        let _ = self.storage_wrapper.storage.store(CONFIG_KEY, &config, format::Json);
    }
    pub fn pre_game_loop(&mut self) {
        if let Some(music_handle) = self.music_handle.as_ref() {
            music_handle.set_volume(GAME_MUSIC_VOLUME);
            if self.config.music {
                music_handle.play();
            }
        }
    }
    pub fn post_game_loop(&mut self) {
        if let Some(music_handle) = self.music_handle.as_ref() {
            music_handle.set_volume(MENU_MUSIC_VOLUME);
        }
    }
    pub fn has_instance(&self) -> bool {
        self.instance.is_some()
    }
    pub fn instantiate(&mut self) {
        let seed = self.rng_seed_source.next_seed();
        self.frontend.log_rng_seed(seed);
        let rng = Isaac64Rng::seed_from_u64(seed);
        self.instance = Some(GameInstance::new(&self.game_config, rng));
    }
    pub fn save_instance(&mut self) {
        log::info!("saving game...");
        if let Some(instance) = self.instance.as_ref() {
            self.storage_wrapper.save_instance(instance);
        } else {
            self.storage_wrapper.clear_instance();
        }
    }
    pub fn clear_instance(&mut self) {
        self.instance = None;
        self.storage_wrapper.clear_instance();
        self.music_handle = None;
    }
    pub fn instance(&self) -> Option<&GameInstance> {
        self.instance.as_ref()
    }
    pub fn initial_aim_coord(&self, screen_coord_of_mouse: ScreenCoord) -> Result<ScreenCoord, NoGameInstance> {
        if let Some(instance) = self.instance.as_ref() {
            if self.last_aim_with_mouse {
                Ok(screen_coord_of_mouse)
            } else {
                let player_coord = GameCoord::of_player(instance.game.player_info());
                let screen_coord = GameCoordToScreenCoord {
                    game_coord: player_coord,
                    player_coord,
                }
                .compute();
                Ok(screen_coord)
            }
        } else {
            Err(NoGameInstance)
        }
    }
}

pub struct NoGameInstance;

pub struct AimEventRoutine<S: Storage, A: AudioPlayer> {
    s: PhantomData<S>,
    a: PhantomData<A>,
    screen_coord: ScreenCoord,
    duration: Duration,
}

impl<S: Storage, A: AudioPlayer> AimEventRoutine<S, A> {
    pub fn new(screen_coord: ScreenCoord) -> Self {
        Self {
            s: PhantomData,
            a: PhantomData,
            screen_coord,
            duration: Duration::from_millis(0),
        }
    }
}

impl<S: Storage, A: AudioPlayer> EventRoutine for AimEventRoutine<S, A> {
    type Return = Option<Coord>;
    type Data = GameData<S, A>;
    type View = GameView;
    type Event = CommonEvent;

    fn handle<EP>(self, data: &mut Self::Data, view: &Self::View, event_or_peek: EP) -> Handled<Self::Return, Self>
    where
        EP: EventOrPeek<Event = Self::Event>,
    {
        enum Aim {
            Mouse { coord: Coord, press: bool },
            KeyboardDirection(CardinalDirection),
            KeyboardFinalise,
            Cancel,
            Ignore,
            Frame(Duration),
        }
        let last_aim_with_mouse = &mut data.last_aim_with_mouse;
        let controls = &data.controls;
        let audio_player = &data.audio_player;
        let audio_table = &data.audio_table;
        let game_config = &data.game_config;
        let current_music_handle = &mut data.music_handle;
        let config = &data.config;
        if let Some(instance) = data.instance.as_mut() {
            event_or_peek_with_handled(event_or_peek, self, |mut s, event| {
                *last_aim_with_mouse = false;
                let aim = match event {
                    CommonEvent::Input(input) => match input {
                        Input::Keyboard(keyboard_input) => {
                            if let Some(app_input) = controls.get(keyboard_input) {
                                match app_input {
                                    AppInput::Move(direction) => Aim::KeyboardDirection(direction),
                                    AppInput::Wait | AppInput::Tech | AppInput::Ability(_) => Aim::Ignore,
                                }
                            } else {
                                match keyboard_input {
                                    keys::RETURN => Aim::KeyboardFinalise,
                                    keys::ESCAPE => Aim::Cancel,
                                    _ => Aim::Ignore,
                                }
                            }
                        }
                        Input::Mouse(mouse_input) => match mouse_input {
                            MouseInput::MouseMove { coord, .. } => Aim::Mouse { coord, press: false },
                            MouseInput::MousePress {
                                coord,
                                button: MouseButton::Left,
                            } => Aim::Mouse { coord, press: true },
                            MouseInput::MousePress {
                                button: MouseButton::Right,
                                ..
                            } => Aim::Cancel,
                            _ => Aim::Ignore,
                        },
                    },
                    CommonEvent::Frame(since_last) => Aim::Frame(since_last),
                };
                match aim {
                    Aim::KeyboardFinalise => Handled::Return(Some(s.screen_coord.0 / 2)),
                    Aim::KeyboardDirection(direction) => {
                        s.screen_coord.0 += direction.coord() * 2;
                        Handled::Continue(s)
                    }
                    Aim::Mouse { coord, press } => {
                        s.screen_coord = ScreenCoord(view.absolute_coord_to_game_relative_screen_coord(coord));
                        if press {
                            *last_aim_with_mouse = true;
                            Handled::Return(Some(s.screen_coord.0 / 2))
                        } else {
                            Handled::Continue(s)
                        }
                    }
                    Aim::Cancel => Handled::Return(None),
                    Aim::Ignore => Handled::Continue(s),
                    Aim::Frame(since_last) => {
                        let game_control_flow = instance.game.handle_tick(since_last, game_config);
                        assert!(game_control_flow.is_none(), "meaningful event while aiming");
                        let mut event_context = EffectContext {
                            rng: &mut instance.rng,
                            screen_shake: &mut instance.screen_shake,
                            current_music: &mut instance.current_music,
                            current_music_handle,
                            audio_player,
                            audio_table,
                            player_coord: GameCoord::of_player(instance.game.player_info()),
                            config,
                        };
                        event_context.next_frame();
                        for event in instance.game.events() {
                            event_context.handle_event(event);
                        }
                        s.duration += since_last;
                        Handled::Continue(s)
                    }
                }
            })
        } else {
            Handled::Return(None)
        }
    }

    fn view<F, C>(&self, data: &Self::Data, view: &mut Self::View, context: ViewContext<C>, frame: &mut F)
    where
        F: Frame,
        C: ColModify,
    {
        if let Some(instance) = data.instance.as_ref() {
            view.view(
                GameToRender {
                    game: &instance.game,
                    status: GameStatus::Playing,
                    mouse_coord: Some(self.screen_coord.0),
                    mode: Mode::Aim {
                        blink_duration: self.duration,
                        target: self.screen_coord.0,
                    },
                    action_error: None,
                },
                context,
                frame,
            );
        }
    }
}

pub struct GameEventRoutine<S: Storage, A: AudioPlayer> {
    s: PhantomData<S>,
    a: PhantomData<A>,
    injected_inputs: Vec<InjectedInput>,
    mouse_coord: Coord,
    action_error: Option<ActionError>,
}

impl<S: Storage, A: AudioPlayer> GameEventRoutine<S, A> {
    pub fn new() -> Self {
        Self::new_injecting_inputs(Vec::new())
    }
    pub fn new_injecting_inputs(injected_inputs: Vec<InjectedInput>) -> Self {
        Self {
            s: PhantomData,
            a: PhantomData,
            injected_inputs,
            mouse_coord: Coord::new(-1, -1),
            action_error: None,
        }
    }
}

pub enum GameReturn {
    Pause,
    Aim,
    GameOver,
}

impl<S: Storage, A: AudioPlayer> EventRoutine for GameEventRoutine<S, A> {
    type Return = GameReturn;
    type Data = GameData<S, A>;
    type View = GameView;
    type Event = CommonEvent;

    fn handle<EP>(mut self, data: &mut Self::Data, _view: &Self::View, event_or_peek: EP) -> Handled<Self::Return, Self>
    where
        EP: EventOrPeek<Event = Self::Event>,
    {
        let storage_wrapper = &mut data.storage_wrapper;
        let audio_player = &data.audio_player;
        let audio_table = &data.audio_table;
        let game_config = &data.game_config;
        let current_music_handle = &mut data.music_handle;
        let config = &data.config;
        if let Some(instance) = data.instance.as_mut() {
            let player_coord = GameCoord::of_player(instance.game.player_info());
            for injected_input in self.injected_inputs.drain(..) {
                match injected_input {
                    InjectedInput::Tech(coord) => {
                        let game_control_flow =
                            instance.game.handle_input(GameInput::TechWithCoord(coord), game_config);
                        match game_control_flow {
                            Err(error) => self.action_error = Some(error),
                            Ok(None) => self.action_error = None,
                            Ok(Some(game_control_flow)) => match game_control_flow {
                                GameControlFlow::GameOver => return Handled::Return(GameReturn::GameOver),
                            },
                        }
                    }
                }
            }
            let controls = &data.controls;
            event_or_peek_with_handled(event_or_peek, self, |mut s, event| match event {
                CommonEvent::Input(input) => {
                    match input {
                        Input::Keyboard(keyboard_input) => {
                            if keyboard_input == keys::ESCAPE {
                                return Handled::Return(GameReturn::Pause);
                            }
                            if !instance.game.is_gameplay_blocked() {
                                if let Some(app_input) = controls.get(keyboard_input) {
                                    let game_control_flow = match app_input {
                                        AppInput::Move(direction) => {
                                            instance.game.handle_input(GameInput::Walk(direction), game_config)
                                        }
                                        AppInput::Tech => {
                                            if let Some(&next_tech) = instance.game.player().tech.peek() {
                                                if next_tech.requires_aim() {
                                                    return Handled::Return(GameReturn::Aim);
                                                } else {
                                                    instance.game.handle_input(GameInput::Tech, game_config)
                                                }
                                            } else {
                                                return Handled::Continue(s);
                                            }
                                        }
                                        AppInput::Wait => instance.game.handle_input(GameInput::Wait, game_config),
                                        AppInput::Ability(n) => {
                                            instance.game.handle_input(GameInput::Ability(n), game_config)
                                        }
                                    };
                                    match game_control_flow {
                                        Err(error) => s.action_error = Some(error),
                                        Ok(None) => s.action_error = None,
                                        Ok(Some(game_control_flow)) => match game_control_flow {
                                            GameControlFlow::GameOver => return Handled::Return(GameReturn::GameOver),
                                        },
                                    }
                                }
                            }
                        }
                        Input::Mouse(mouse_input) => match mouse_input {
                            MouseInput::MouseMove { coord, .. } => {
                                s.mouse_coord = coord;
                            }
                            _ => (),
                        },
                    }
                    Handled::Continue(s)
                }
                CommonEvent::Frame(period) => {
                    let maybe_control_flow = instance.game.handle_tick(period, game_config);
                    let mut event_context = EffectContext {
                        rng: &mut instance.rng,
                        screen_shake: &mut instance.screen_shake,
                        current_music: &mut instance.current_music,
                        current_music_handle,
                        audio_player,
                        audio_table,
                        player_coord,
                        config,
                    };
                    event_context.next_frame();
                    for event in instance.game.events() {
                        event_context.handle_event(event);
                    }
                    if let Some(game_control_flow) = maybe_control_flow {
                        match game_control_flow {
                            GameControlFlow::GameOver => return Handled::Return(GameReturn::GameOver),
                        }
                    }
                    Handled::Continue(s)
                }
            })
        } else {
            storage_wrapper.clear_instance();
            Handled::Continue(self)
        }
    }

    fn view<F, C>(&self, data: &Self::Data, view: &mut Self::View, context: ViewContext<C>, frame: &mut F)
    where
        F: Frame,
        C: ColModify,
    {
        if let Some(instance) = data.instance.as_ref() {
            view.view(
                GameToRender {
                    game: &instance.game,
                    status: GameStatus::Playing,
                    mouse_coord: Some(self.mouse_coord),
                    mode: Mode::Normal,
                    action_error: self.action_error,
                },
                context,
                frame,
            );
        }
    }
}

pub struct GameOverEventRoutine<S: Storage, A: AudioPlayer> {
    s: PhantomData<S>,
    a: PhantomData<A>,
    duration: Duration,
}

impl<S: Storage, A: AudioPlayer> GameOverEventRoutine<S, A> {
    pub fn new() -> Self {
        Self {
            s: PhantomData,
            a: PhantomData,
            duration: Duration::from_millis(0),
        }
    }
}

impl<S: Storage, A: AudioPlayer> EventRoutine for GameOverEventRoutine<S, A> {
    type Return = ();
    type Data = GameData<S, A>;
    type View = GameView;
    type Event = CommonEvent;

    fn handle<EP>(self, data: &mut Self::Data, _view: &Self::View, event_or_peek: EP) -> Handled<Self::Return, Self>
    where
        EP: EventOrPeek<Event = Self::Event>,
    {
        let game_config = &data.game_config;
        let audio_player = &data.audio_player;
        let audio_table = &data.audio_table;
        let current_music_handle = &mut data.music_handle;
        let config = &data.config;
        if let Some(instance) = data.instance.as_mut() {
            event_or_peek_with_handled(event_or_peek, self, |mut s, event| match event {
                CommonEvent::Input(input) => match input {
                    Input::Keyboard(_) => Handled::Return(()),
                    Input::Mouse(_) => Handled::Continue(s),
                },
                CommonEvent::Frame(period) => {
                    s.duration += period;
                    const NPC_TURN_PERIOD: Duration = Duration::from_millis(100);
                    if s.duration > NPC_TURN_PERIOD {
                        s.duration -= NPC_TURN_PERIOD;
                        instance.game.handle_npc_turn();
                    }
                    let _ = instance.game.handle_tick(period, game_config);
                    let mut event_context = EffectContext {
                        rng: &mut instance.rng,
                        screen_shake: &mut instance.screen_shake,
                        current_music: &mut instance.current_music,
                        current_music_handle,
                        audio_player,
                        audio_table,
                        player_coord: GameCoord::of_player(instance.game.player_info()),
                        config,
                    };
                    event_context.next_frame();
                    for event in instance.game.events() {
                        event_context.handle_event(event);
                    }
                    Handled::Continue(s)
                }
            })
        } else {
            Handled::Return(())
        }
    }
    fn view<F, C>(&self, data: &Self::Data, view: &mut Self::View, context: ViewContext<C>, frame: &mut F)
    where
        F: Frame,
        C: ColModify,
    {
        if let Some(instance) = data.instance.as_ref() {
            view.view(
                GameToRender {
                    game: &instance.game,
                    status: GameStatus::Over,
                    mouse_coord: None,
                    mode: Mode::Normal,
                    action_error: None,
                },
                context,
                frame,
            );
        }
    }
}
