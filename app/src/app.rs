use crate::audio::Audio;
use crate::controls::Controls;
use crate::depth;
use crate::frontend::Frontend;
use crate::game::{
    AbilityChoice, AimEventRoutine, ExamineEventRoutine, GameData, GameEventRoutine, GameOverEventRoutine, GameReturn,
    GameStatus, InjectedInput, ScreenCoord,
};
pub use crate::game::{GameConfig, Omniscient, RngSeed};
use crate::render::{GameToRender, GameView, Mode};
use crate::ui;
use chargrid::input::*;
use chargrid::*;
use common_event::*;
use decorator::*;
use event_routine::*;
use game::player::Ability;
use general_audio::AudioPlayer;
use general_storage::Storage;
use maplit::hashmap;
use menu::{fade_spec, FadeMenuInstanceView, MenuEntryStringFn, MenuEntryToRender, MenuInstanceChoose};
use render::{ColModifyDefaultForeground, ColModifyMap, Coord, Rgb24, Style};
use std::collections::HashMap;
use std::marker::PhantomData;

#[derive(Clone, Copy)]
enum MainMenuType {
    Init,
    Pause,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum MainMenuEntry {
    NewGame,
    Resume,
    Quit,
    Save,
    SaveQuit,
    Clear,
    Options,
    Story,
    Keybindings,
    EndText,
}

impl MainMenuEntry {
    fn init(frontend: Frontend) -> menu::MenuInstance<Self> {
        use MainMenuEntry::*;
        let (items, hotkeys) = match frontend {
            Frontend::Graphical | Frontend::AnsiTerminal => (
                vec![NewGame, Options, Keybindings, Story, Quit],
                hashmap!['n' => NewGame, 'o' => Options, 'k' => Keybindings, 'b' => Story, 'q' => Quit],
            ),
            Frontend::Web => (
                vec![NewGame, Options, Keybindings, Story],
                hashmap!['n' => NewGame, 'o' => Options, 'k' => Keybindings, 'b' => Story],
            ),
        };
        menu::MenuInstanceBuilder {
            items,
            selected_index: 0,
            hotkeys: Some(hotkeys),
        }
        .build()
        .unwrap()
    }
    fn won(frontend: Frontend) -> menu::MenuInstance<Self> {
        use MainMenuEntry::*;
        let (items, hotkeys) = match frontend {
            Frontend::Graphical | Frontend::AnsiTerminal => (
                vec![NewGame, Options, Keybindings, Story, EndText, Quit],
                hashmap!['n' => NewGame, 'o' => Options, 'k' => Keybindings, 'b' => Story, 'e' => EndText, 'q' => Quit],
            ),
            Frontend::Web => (
                vec![NewGame, Options, Keybindings, Story, EndText],
                hashmap!['n' => NewGame, 'o' => Options, 'k' => Keybindings, 'b' => Story, 'e' => EndText],
            ),
        };
        menu::MenuInstanceBuilder {
            items,
            selected_index: 0,
            hotkeys: Some(hotkeys),
        }
        .build()
        .unwrap()
    }
    fn pause(frontend: Frontend) -> menu::MenuInstance<Self> {
        use MainMenuEntry::*;
        let (items, hotkeys) = match frontend {
            Frontend::Graphical | Frontend::AnsiTerminal => (
                vec![Resume, SaveQuit, NewGame, Options, Keybindings, Story, Clear],
                hashmap!['r' => Resume, 'q' => SaveQuit, 'o' => Options, 'k' => Keybindings, 'b'=> Story, 'n' => NewGame, 'c' => Clear],
            ),
            Frontend::Web => (
                vec![Resume, Save, NewGame, Options, Story, Clear],
                hashmap!['r' => Resume, 's' => Save, 'o' => Options, 'k' => Keybindings, 'b' => Story, 'n' => NewGame, 'c' => Clear],
            ),
        };
        menu::MenuInstanceBuilder {
            items,
            selected_index: 0,
            hotkeys: Some(hotkeys),
        }
        .build()
        .unwrap()
    }
}

struct AppData<S: Storage, A: AudioPlayer> {
    frontend: Frontend,
    game: GameData<S, A>,
    main_menu: menu::MenuInstanceChooseOrEscape<MainMenuEntry>,
    main_menu_type: MainMenuType,
    options_menu: menu::MenuInstanceChooseOrEscape<OrBack<OptionsMenuEntry>>,
    level_change_menu: Option<menu::MenuInstanceChooseOrEscape<Ability>>,
    last_mouse_coord: Coord,
    env: Box<dyn Env>,
    won: bool,
}

struct AppView {
    game: GameView,
    main_menu: FadeMenuInstanceView,
    options_menu: FadeMenuInstanceView,
    level_change_menu: FadeMenuInstanceView,
}

impl<S: Storage, A: AudioPlayer> AppData<S, A> {
    fn new(
        game_config: GameConfig,
        frontend: Frontend,
        controls: Controls,
        storage: S,
        save_key: String,
        audio_player: A,
        rng_seed: RngSeed,
        fullscreen: Option<Fullscreen>,
        env: Box<dyn Env>,
    ) -> Self {
        let mut game_data = GameData::new(
            game_config,
            controls,
            storage,
            save_key,
            audio_player,
            rng_seed,
            frontend,
        );
        if env.fullscreen_supported() {
            let mut config = game_data.config();
            if fullscreen.is_some() {
                config.fullscreen = true;
            }
            env.set_fullscreen_init(config.fullscreen);
            game_data.set_config(config);
        }
        Self {
            options_menu: OptionsMenuEntry::instance(&env),
            level_change_menu: None,
            frontend,
            game: game_data,
            main_menu: MainMenuEntry::init(frontend).into_choose_or_escape(),
            main_menu_type: MainMenuType::Init,
            last_mouse_coord: Coord::new(0, 0),
            env,
            won: false,
        }
    }
}

impl AppView {
    fn new() -> Self {
        use fade_spec::*;
        let spec = Spec {
            normal: Style {
                to: To {
                    foreground: Rgb24::new(127, 127, 127),
                    background: Rgb24::new(0, 0, 0),
                    bold: false,
                    underline: false,
                },
                from: From::current(),
                durations: Durations {
                    foreground: Duration::from_millis(127),
                    background: Duration::from_millis(127),
                },
            },
            selected: Style {
                to: To {
                    foreground: Rgb24::new(255, 255, 255),
                    background: Rgb24::new(87, 87, 87),
                    bold: true,
                    underline: false,
                },
                from: From {
                    foreground: FromCol::Rgb24(Rgb24::new(0, 0, 0)),
                    background: FromCol::Rgb24(Rgb24::new(255, 255, 255)),
                },
                durations: Durations {
                    foreground: Duration::from_millis(63),
                    background: Duration::from_millis(127),
                },
            },
        };
        Self {
            game: GameView::new(),
            main_menu: FadeMenuInstanceView::new(spec.clone()),
            options_menu: FadeMenuInstanceView::new(spec.clone()),
            level_change_menu: FadeMenuInstanceView::new(spec.clone()),
        }
    }
}

impl Default for AppView {
    fn default() -> Self {
        Self::new()
    }
}

struct SelectGame<S: Storage, A: AudioPlayer> {
    s: PhantomData<S>,
    a: PhantomData<A>,
}
impl<S: Storage, A: AudioPlayer> SelectGame<S, A> {
    fn new() -> Self {
        Self {
            s: PhantomData,
            a: PhantomData,
        }
    }
}
impl<S: Storage, A: AudioPlayer> DataSelector for SelectGame<S, A> {
    type DataInput = AppData<S, A>;
    type DataOutput = GameData<S, A>;
    fn data<'a>(&self, input: &'a Self::DataInput) -> &'a Self::DataOutput {
        &input.game
    }
    fn data_mut<'a>(&self, input: &'a mut Self::DataInput) -> &'a mut Self::DataOutput {
        &mut input.game
    }
}
impl<S: Storage, A: AudioPlayer> ViewSelector for SelectGame<S, A> {
    type ViewInput = AppView;
    type ViewOutput = GameView;
    fn view<'a>(&self, input: &'a Self::ViewInput) -> &'a Self::ViewOutput {
        &input.game
    }
    fn view_mut<'a>(&self, input: &'a mut Self::ViewInput) -> &'a mut Self::ViewOutput {
        &mut input.game
    }
}
impl<S: Storage, A: AudioPlayer> Selector for SelectGame<S, A> {}

struct SelectMainMenu<S: Storage, A: AudioPlayer> {
    s: PhantomData<S>,
    a: PhantomData<A>,
}
impl<S: Storage, A: AudioPlayer> SelectMainMenu<S, A> {
    fn new() -> Self {
        Self {
            s: PhantomData,
            a: PhantomData,
        }
    }
}
impl<S: Storage, A: AudioPlayer> ViewSelector for SelectMainMenu<S, A> {
    type ViewInput = AppView;
    type ViewOutput = FadeMenuInstanceView;
    fn view<'a>(&self, input: &'a Self::ViewInput) -> &'a Self::ViewOutput {
        &input.main_menu
    }
    fn view_mut<'a>(&self, input: &'a mut Self::ViewInput) -> &'a mut Self::ViewOutput {
        &mut input.main_menu
    }
}
impl<S: Storage, A: AudioPlayer> DataSelector for SelectMainMenu<S, A> {
    type DataInput = AppData<S, A>;
    type DataOutput = menu::MenuInstanceChooseOrEscape<MainMenuEntry>;
    fn data<'a>(&self, input: &'a Self::DataInput) -> &'a Self::DataOutput {
        &input.main_menu
    }
    fn data_mut<'a>(&self, input: &'a mut Self::DataInput) -> &'a mut Self::DataOutput {
        &mut input.main_menu
    }
}
impl<S: Storage, A: AudioPlayer> Selector for SelectMainMenu<S, A> {}

struct DecorateMainMenu<S, A> {
    s: PhantomData<S>,
    a: PhantomData<A>,
}
impl<S: Storage, A: AudioPlayer> DecorateMainMenu<S, A> {
    fn new() -> Self {
        Self {
            s: PhantomData,
            a: PhantomData,
        }
    }
}

struct LevelChangeMenu<'b, 'e, 'v, E: EventRoutine>(&'b mut EventRoutineView<'e, 'v, E>);
impl<'b, 'a, 'e, 'v, S, A, E> View<&'a AppData<S, A>> for LevelChangeMenu<'b, 'e, 'v, E>
where
    S: Storage,
    A: AudioPlayer,
    E: EventRoutine<View = AppView, Data = AppData<S, A>>,
{
    fn view<F: Frame, C: ColModify>(&mut self, app_data: &'a AppData<S, A>, context: ViewContext<C>, frame: &mut F) {
        text::StringView::new(
            Style::new().with_foreground(Rgb24::new_grey(255)).with_bold(true),
            text::wrap::Word::new(),
        )
        .view(
            "Good work soldier.\nYou get an abiltiy.\nChoose now:",
            context.add_offset(Coord::new(1, 1)),
            frame,
        );
        self.0.view(app_data, context.add_offset(Coord::new(1, 5)), frame);
    }
}

struct InitMenu<'e, 'v, E: EventRoutine>(EventRoutineView<'e, 'v, E>);
impl<'a, 'e, 'v, S, A, E> View<&'a AppData<S, A>> for InitMenu<'e, 'v, E>
where
    S: Storage,
    A: AudioPlayer,
    E: EventRoutine<View = AppView, Data = AppData<S, A>>,
{
    fn view<F: Frame, C: ColModify>(&mut self, app_data: &'a AppData<S, A>, context: ViewContext<C>, frame: &mut F) {
        text::StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new(0, 255, 0)).with_bold(true)).view(
            "slime99",
            context.add_offset(Coord::new(1, 1)),
            frame,
        );
        self.0.view(app_data, context.add_offset(Coord::new(1, 3)), frame);
    }
}

struct TextOverlay<S, A> {
    s: PhantomData<S>,
    a: PhantomData<A>,
    text: Vec<text::RichTextPartOwned>,
}
impl<S: Storage, A: AudioPlayer> TextOverlay<S, A> {
    fn new(text: Vec<text::RichTextPartOwned>) -> Self {
        Self {
            s: PhantomData,
            a: PhantomData,
            text,
        }
    }
}
impl<S: Storage, A: AudioPlayer> EventRoutine for TextOverlay<S, A> {
    type Return = ();
    type Data = AppData<S, A>;
    type View = AppView;
    type Event = CommonEvent;
    fn handle<EP>(self, _data: &mut Self::Data, _view: &Self::View, event_or_peek: EP) -> Handled<Self::Return, Self>
    where
        EP: EventOrPeek<Event = Self::Event>,
    {
        event_or_peek_with_handled(event_or_peek, self, |s, event| match event {
            CommonEvent::Input(input) => match input {
                Input::Keyboard(_) => Handled::Return(()),
                Input::Mouse(_) => Handled::Continue(s),
            },
            CommonEvent::Frame(_) => Handled::Continue(s),
        })
    }
    fn view<F, C>(&self, data: &Self::Data, view: &mut Self::View, context: ViewContext<C>, frame: &mut F)
    where
        F: Frame,
        C: ColModify,
    {
        if let Some(instance) = data.game.instance() {
            AlignView {
                alignment: Alignment::centre(),
                view: FillBackgroundView {
                    rgb24: Rgb24::new_grey(0),
                    view: BorderView {
                        style: &BorderStyle {
                            padding: BorderPadding::all(1),
                            ..Default::default()
                        },
                        view: BoundView {
                            size: Size::new(40, 16),
                            view: text::RichTextView::new(text::wrap::Word::new()),
                        },
                    },
                },
            }
            .view(
                self.text.iter().map(|t| t.as_rich_text_part()),
                context.add_depth(depth::GAME_MAX + 1),
                frame,
            );
            view.game.view(
                GameToRender {
                    game: instance.game(),
                    status: GameStatus::Playing,
                    mouse_coord: None,
                    mode: Mode::Normal,
                    action_error: None,
                },
                context.compose_col_modify(
                    ColModifyDefaultForeground(Rgb24::new_grey(255))
                        .compose(ColModifyMap(|col: Rgb24| col.saturating_scalar_mul_div(1, 3))),
                ),
                frame,
            );
        } else {
            AlignView {
                alignment: Alignment::centre(),
                view: FillBackgroundView {
                    rgb24: Rgb24::new_grey(0),
                    view: BoundView {
                        size: Size::new(50, 20),
                        view: text::RichTextView::new(text::wrap::Word::new()),
                    },
                },
            }
            .view(self.text.iter().map(|t| t.as_rich_text_part()), context, frame);
        }
    }
}

impl<S: Storage, A: AudioPlayer> Decorate for DecorateMainMenu<S, A> {
    type View = AppView;
    type Data = AppData<S, A>;
    fn view<E, F, C>(
        data: &Self::Data,
        mut event_routine_view: EventRoutineView<E>,
        context: ViewContext<C>,
        frame: &mut F,
    ) where
        E: EventRoutine<Data = Self::Data, View = Self::View>,
        F: Frame,
        C: ColModify,
    {
        if let Some(instance) = data.game.instance() {
            AlignView {
                alignment: Alignment::centre(),
                view: FillBackgroundView {
                    rgb24: Rgb24::new_grey(0),
                    view: BorderView {
                        style: &BorderStyle::new(),
                        view: &mut event_routine_view,
                    },
                },
            }
            .view(data, context.add_depth(depth::GAME_MAX + 1), frame);
            event_routine_view.view.game.view(
                GameToRender {
                    game: instance.game(),
                    status: GameStatus::Playing,
                    mouse_coord: None,
                    mode: Mode::Normal,
                    action_error: None,
                },
                context.compose_col_modify(
                    ColModifyDefaultForeground(Rgb24::new_grey(255))
                        .compose(ColModifyMap(|col: Rgb24| col.saturating_scalar_mul_div(1, 3))),
                ),
                frame,
            );
        } else {
            AlignView {
                view: InitMenu(event_routine_view),
                alignment: Alignment::centre(),
            }
            .view(&data, context, frame);
        }
    }
}

struct DecorateGame<S, A> {
    s: PhantomData<S>,
    a: PhantomData<A>,
}
impl<S, A> DecorateGame<S, A>
where
    S: Storage,
    A: AudioPlayer,
{
    fn new() -> Self {
        Self {
            s: PhantomData,
            a: PhantomData,
        }
    }
}

impl<S: Storage, A: AudioPlayer> Decorate for DecorateGame<S, A> {
    type View = AppView;
    type Data = AppData<S, A>;
    fn view<E, F, C>(
        data: &Self::Data,
        mut event_routine_view: EventRoutineView<E>,
        context: ViewContext<C>,
        frame: &mut F,
    ) where
        E: EventRoutine<Data = Self::Data, View = Self::View>,
        F: Frame,
        C: ColModify,
    {
        event_routine_view.view(data, context, frame);
    }
}

struct Quit;

struct MouseTracker<S: Storage, A: AudioPlayer, E: EventRoutine> {
    s: PhantomData<S>,
    a: PhantomData<A>,
    e: E,
}

impl<S: Storage, A: AudioPlayer, E: EventRoutine> MouseTracker<S, A, E> {
    fn new(e: E) -> Self {
        Self {
            s: PhantomData,
            a: PhantomData,
            e,
        }
    }
}

impl<S: Storage, A: AudioPlayer, E: EventRoutine<Data = AppData<S, A>, Event = CommonEvent>> EventRoutine
    for MouseTracker<S, A, E>
{
    type Return = E::Return;
    type View = E::View;
    type Data = AppData<S, A>;
    type Event = CommonEvent;

    fn handle<EP>(self, data: &mut Self::Data, view: &Self::View, event_or_peek: EP) -> Handled<Self::Return, Self>
    where
        EP: EventOrPeek<Event = Self::Event>,
    {
        event_or_peek.with(
            (self, data),
            |(s, data), event| {
                if let CommonEvent::Input(Input::Mouse(MouseInput::MouseMove { coord, .. })) = event {
                    data.last_mouse_coord = coord;
                }
                s.e.handle(data, view, event_routine::Event::new(event))
                    .map_continue(|e| Self {
                        s: PhantomData,
                        a: PhantomData,
                        e,
                    })
            },
            |(s, data)| {
                s.e.handle(data, view, event_routine::Peek::new())
                    .map_continue(|e| Self {
                        s: PhantomData,
                        a: PhantomData,
                        e,
                    })
            },
        )
    }
    fn view<F, C>(&self, data: &Self::Data, view: &mut Self::View, context: ViewContext<C>, frame: &mut F)
    where
        F: Frame,
        C: ColModify,
    {
        self.e.view(data, view, context, frame)
    }
}

struct SelectLevelChangeMenu<S: Storage, A: AudioPlayer> {
    s: PhantomData<S>,
    a: PhantomData<A>,
}
impl<S: Storage, A: AudioPlayer> SelectLevelChangeMenu<S, A> {
    fn new() -> Self {
        Self {
            s: PhantomData,
            a: PhantomData,
        }
    }
}
impl<S: Storage, A: AudioPlayer> ViewSelector for SelectLevelChangeMenu<S, A> {
    type ViewInput = AppView;
    type ViewOutput = FadeMenuInstanceView;
    fn view<'a>(&self, input: &'a Self::ViewInput) -> &'a Self::ViewOutput {
        &input.level_change_menu
    }
    fn view_mut<'a>(&self, input: &'a mut Self::ViewInput) -> &'a mut Self::ViewOutput {
        &mut input.level_change_menu
    }
}
impl<S: Storage, A: AudioPlayer> DataSelector for SelectLevelChangeMenu<S, A> {
    type DataInput = AppData<S, A>;
    type DataOutput = menu::MenuInstanceChooseOrEscape<Ability>;
    fn data<'a>(&self, input: &'a Self::DataInput) -> &'a Self::DataOutput {
        input.level_change_menu.as_ref().unwrap()
    }
    fn data_mut<'a>(&self, input: &'a mut Self::DataInput) -> &'a mut Self::DataOutput {
        input.level_change_menu.as_mut().unwrap()
    }
}
impl<S: Storage, A: AudioPlayer> Selector for SelectLevelChangeMenu<S, A> {}

struct DecorateLevelChangeMenu<S, A> {
    s: PhantomData<S>,
    a: PhantomData<A>,
}
impl<S: Storage, A: AudioPlayer> DecorateLevelChangeMenu<S, A> {
    fn new() -> Self {
        Self {
            s: PhantomData,
            a: PhantomData,
        }
    }
}
impl<S: Storage, A: AudioPlayer> Decorate for DecorateLevelChangeMenu<S, A> {
    type View = AppView;
    type Data = AppData<S, A>;
    fn view<E, F, C>(
        data: &Self::Data,
        mut event_routine_view: EventRoutineView<E>,
        context: ViewContext<C>,
        frame: &mut F,
    ) where
        E: EventRoutine<Data = Self::Data, View = Self::View>,
        F: Frame,
        C: ColModify,
    {
        if let Some(instance) = data.game.instance() {
            AlignView {
                alignment: Alignment::centre(),
                view: FillBackgroundView {
                    rgb24: Rgb24::new_grey(0),
                    view: BorderView {
                        style: &BorderStyle::new(),
                        view: PadView {
                            size: Size::new(0, 1),
                            view: LevelChangeMenu(&mut event_routine_view),
                        },
                    },
                },
            }
            .view(data, context.add_depth(depth::GAME_MAX + 1), frame);
            event_routine_view.view.game.view(
                GameToRender {
                    game: instance.game(),
                    status: GameStatus::Playing,
                    mouse_coord: None,
                    mode: Mode::Normal,
                    action_error: None,
                },
                context.compose_col_modify(
                    ColModifyDefaultForeground(Rgb24::new_grey(255))
                        .compose(ColModifyMap(|col: Rgb24| col.saturating_scalar_mul_div(1, 3))),
                ),
                frame,
            );
        } else {
            AlignView {
                view: InitMenu(event_routine_view),
                alignment: Alignment::centre(),
            }
            .view(&data, context, frame);
        }
    }
}

fn level_change_menu<S: Storage, A: AudioPlayer>(
    AbilityChoice(choices): AbilityChoice,
) -> impl EventRoutine<Return = Result<Ability, menu::Escape>, Data = AppData<S, A>, View = AppView, Event = CommonEvent>
{
    SideEffectThen::new_with_view(|data: &mut AppData<S, A>, _: &_| {
        data.level_change_menu = Some(
            menu::MenuInstanceBuilder {
                hotkeys: Some(
                    choices
                        .iter()
                        .enumerate()
                        .map(|(i, choice)| (std::char::from_digit(i as u32 + 1, 10).unwrap(), choice.clone()))
                        .collect::<HashMap<_, _>>(),
                ),
                items: choices,
                selected_index: 0,
            }
            .build()
            .unwrap()
            .into_choose_or_escape(),
        );
        let menu_entry_string = MenuEntryStringFn::new(move |entry: MenuEntryToRender<Ability>, buf: &mut String| {
            use std::fmt::Write;
            write!(buf, "({}) ", entry.index + 1).unwrap();
            ui::write_abiilty(*entry.entry, buf);
        });
        menu::FadeMenuInstanceRoutine::new(menu_entry_string)
            .select(SelectLevelChangeMenu::new())
            .decorated(DecorateLevelChangeMenu::new())
    })
}

#[derive(Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq)]
enum OrBack<T> {
    Selection(T),
    Back,
}

#[derive(Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq)]
enum OptionsMenuEntry {
    ToggleMusic,
    ToggleSfx,
    ToggleFullscreen,
}

impl OptionsMenuEntry {
    fn instance(env: &Box<dyn Env>) -> menu::MenuInstanceChooseOrEscape<OrBack<OptionsMenuEntry>> {
        use OptionsMenuEntry::*;
        use OrBack::*;
        menu::MenuInstanceBuilder {
            items: if env.fullscreen_supported() {
                vec![
                    Selection(ToggleMusic),
                    Selection(ToggleSfx),
                    Selection(ToggleFullscreen),
                    Back,
                ]
            } else {
                vec![Selection(ToggleMusic), Selection(ToggleSfx), Back]
            },
            selected_index: 0,
            hotkeys: Some(hashmap![
                'm' => Selection(ToggleMusic),
                's' => Selection(ToggleSfx),
                'f' => Selection(ToggleFullscreen),
            ]),
        }
        .build()
        .unwrap()
        .into_choose_or_escape()
    }
}

struct SelectOptionsMenu<S: Storage, A: AudioPlayer> {
    s: PhantomData<S>,
    a: PhantomData<A>,
}
impl<S: Storage, A: AudioPlayer> SelectOptionsMenu<S, A> {
    fn new() -> Self {
        Self {
            s: PhantomData,
            a: PhantomData,
        }
    }
}
impl<S: Storage, A: AudioPlayer> ViewSelector for SelectOptionsMenu<S, A> {
    type ViewInput = AppView;
    type ViewOutput = FadeMenuInstanceView;
    fn view<'a>(&self, input: &'a Self::ViewInput) -> &'a Self::ViewOutput {
        &input.options_menu
    }
    fn view_mut<'a>(&self, input: &'a mut Self::ViewInput) -> &'a mut Self::ViewOutput {
        &mut input.options_menu
    }
}
impl<S: Storage, A: AudioPlayer> DataSelector for SelectOptionsMenu<S, A> {
    type DataInput = AppData<S, A>;
    type DataOutput = menu::MenuInstanceChooseOrEscape<OrBack<OptionsMenuEntry>>;
    fn data<'a>(&self, input: &'a Self::DataInput) -> &'a Self::DataOutput {
        &input.options_menu
    }
    fn data_mut<'a>(&self, input: &'a mut Self::DataInput) -> &'a mut Self::DataOutput {
        &mut input.options_menu
    }
}
impl<S: Storage, A: AudioPlayer> Selector for SelectOptionsMenu<S, A> {}

struct DecorateOptionsMenu<S, A> {
    s: PhantomData<S>,
    a: PhantomData<A>,
}
impl<S: Storage, A: AudioPlayer> DecorateOptionsMenu<S, A> {
    fn new() -> Self {
        Self {
            s: PhantomData,
            a: PhantomData,
        }
    }
}
impl<S: Storage, A: AudioPlayer> Decorate for DecorateOptionsMenu<S, A> {
    type View = AppView;
    type Data = AppData<S, A>;
    fn view<E, F, C>(
        data: &Self::Data,
        mut event_routine_view: EventRoutineView<E>,
        context: ViewContext<C>,
        frame: &mut F,
    ) where
        E: EventRoutine<Data = Self::Data, View = Self::View>,
        F: Frame,
        C: ColModify,
    {
        if let Some(instance) = data.game.instance() {
            AlignView {
                alignment: Alignment::centre(),
                view: FillBackgroundView {
                    rgb24: Rgb24::new_grey(0),
                    view: BorderView {
                        style: &BorderStyle::new(),
                        view: &mut event_routine_view,
                    },
                },
            }
            .view(data, context.add_depth(depth::GAME_MAX + 1), frame);
            event_routine_view.view.game.view(
                GameToRender {
                    game: instance.game(),
                    status: GameStatus::Playing,
                    mouse_coord: None,
                    mode: Mode::Normal,
                    action_error: None,
                },
                context.compose_col_modify(
                    ColModifyDefaultForeground(Rgb24::new_grey(255))
                        .compose(ColModifyMap(|col: Rgb24| col.saturating_scalar_mul_div(1, 3))),
                ),
                frame,
            );
        } else {
            AlignView {
                view: InitMenu(event_routine_view),
                alignment: Alignment::centre(),
            }
            .view(&data, context, frame);
        }
    }
}

fn options_menu<S: Storage, A: AudioPlayer>() -> impl EventRoutine<
    Return = Result<OrBack<OptionsMenuEntry>, menu::Escape>,
    Data = AppData<S, A>,
    View = AppView,
    Event = CommonEvent,
> {
    SideEffectThen::new_with_view(|data: &mut AppData<S, A>, _: &_| {
        let config = data.game.config();
        let fullscreen = data.env.fullscreen();
        let fullscreen_requires_restart = data.env.fullscreen_requires_restart();
        let menu_entry_string = MenuEntryStringFn::new(
            move |entry: MenuEntryToRender<OrBack<OptionsMenuEntry>>, buf: &mut String| {
                use std::fmt::Write;
                use OptionsMenuEntry::*;
                use OrBack::*;
                match entry.entry {
                    Back => write!(buf, "back").unwrap(),
                    Selection(entry) => match entry {
                        ToggleMusic => {
                            write!(buf, "(m) Music enabled [{}]", if config.music { '*' } else { ' ' }).unwrap()
                        }
                        ToggleSfx => write!(buf, "(s) Sfx enabled [{}]", if config.sfx { '*' } else { ' ' }).unwrap(),
                        ToggleFullscreen => {
                            if fullscreen_requires_restart {
                                write!(
                                    buf,
                                    "(f) Fullscreen (requires restart) [{}]",
                                    if fullscreen { '*' } else { ' ' }
                                )
                                .unwrap()
                            } else {
                                write!(buf, "(f) Fullscreen [{}]", if fullscreen { '*' } else { ' ' }).unwrap()
                            }
                        }
                    },
                }
            },
        );
        menu::FadeMenuInstanceRoutine::new(menu_entry_string)
            .select(SelectOptionsMenu::new())
            .decorated(DecorateOptionsMenu::new())
    })
}

fn options_menu_cycle<S: Storage, A: AudioPlayer>(
) -> impl EventRoutine<Return = (), Data = AppData<S, A>, View = AppView, Event = CommonEvent> {
    make_either!(Ei = A | B);
    use OptionsMenuEntry::*;
    use OrBack::*;
    Ei::A(options_menu()).repeat(|choice| match choice {
        Ok(Back) | Err(menu::Escape) => Handled::Return(()),
        Ok(Selection(selection)) => Handled::Continue(Ei::B(SideEffectThen::new_with_view(
            move |data: &mut AppData<S, A>, _: &_| {
                let mut config = data.game.config();
                match selection {
                    ToggleMusic => config.music = !config.music,
                    ToggleSfx => config.sfx = !config.sfx,
                    ToggleFullscreen => {
                        data.env.set_fullscreen(!data.env.fullscreen());
                        config.fullscreen = data.env.fullscreen();
                    }
                }
                data.game.set_config(config);
                options_menu()
            },
        ))),
    })
}

#[derive(Clone, Copy)]
pub struct AutoPlay;

#[derive(Clone, Copy)]
pub struct FirstRun;

fn main_menu<S: Storage, A: AudioPlayer>(
    auto_play: Option<AutoPlay>,
    first_run: Option<FirstRun>,
) -> impl EventRoutine<Return = Result<MainMenuEntry, menu::Escape>, Data = AppData<S, A>, View = AppView, Event = CommonEvent>
{
    make_either!(Ei = A | B | C | D);
    SideEffectThen::new_with_view(move |data: &mut AppData<S, A>, _: &_| {
        if auto_play.is_some() {
            if first_run.is_some() {
                if data.game.has_instance() {
                    Ei::D(story().map(|()| Ok(MainMenuEntry::Resume)))
                } else {
                    Ei::C(story().map(|()| Ok(MainMenuEntry::NewGame)))
                }
            } else {
                if data.game.has_instance() {
                    Ei::A(Value::new(Ok(MainMenuEntry::Resume)))
                } else {
                    Ei::A(Value::new(Ok(MainMenuEntry::NewGame)))
                }
            }
        } else {
            if data.game.has_instance() {
                match data.main_menu_type {
                    MainMenuType::Init => {
                        data.main_menu = MainMenuEntry::pause(data.frontend).into_choose_or_escape();
                        data.main_menu_type = MainMenuType::Pause;
                    }
                    MainMenuType::Pause => (),
                }
            } else {
                if data.won {
                    data.main_menu = MainMenuEntry::won(data.frontend).into_choose_or_escape();
                    data.main_menu_type = MainMenuType::Init;
                } else {
                    if !data.game.is_music_playing() {
                        data.game.loop_music(Audio::Menu, 0.2);
                    }
                    match data.main_menu_type {
                        MainMenuType::Init => (),
                        MainMenuType::Pause => {
                            data.main_menu = MainMenuEntry::init(data.frontend).into_choose_or_escape();
                            data.main_menu_type = MainMenuType::Init;
                        }
                    }
                }
            }
            Ei::B(
                menu::FadeMenuInstanceRoutine::new(MenuEntryStringFn::new(
                    |entry: MenuEntryToRender<MainMenuEntry>, buf: &mut String| {
                        use std::fmt::Write;
                        let s = match entry.entry {
                            MainMenuEntry::NewGame => "(n) New Game",
                            MainMenuEntry::Resume => "(r) Resume",
                            MainMenuEntry::Quit => "(q) Quit",
                            MainMenuEntry::SaveQuit => "(q) Save and Quit",
                            MainMenuEntry::Save => "(s) Save",
                            MainMenuEntry::Clear => "(c) Clear",
                            MainMenuEntry::Options => "(o) Options",
                            MainMenuEntry::Story => "(b) Back Story",
                            MainMenuEntry::Keybindings => "(k) Keybindings",
                            MainMenuEntry::EndText => "(e) End Text",
                        };
                        write!(buf, "{}", s).unwrap();
                    },
                ))
                .select(SelectMainMenu::new())
                .decorated(DecorateMainMenu::new()),
            )
        }
    })
}

fn game<S: Storage, A: AudioPlayer>(
) -> impl EventRoutine<Return = GameReturn, Data = AppData<S, A>, View = AppView, Event = CommonEvent> {
    GameEventRoutine::new()
        .select(SelectGame::new())
        .decorated(DecorateGame::new())
}

fn game_injecting_inputs<S: Storage, A: AudioPlayer>(
    inputs: Vec<InjectedInput>,
) -> impl EventRoutine<Return = GameReturn, Data = AppData<S, A>, View = AppView, Event = CommonEvent> {
    GameEventRoutine::new_injecting_inputs(inputs)
        .select(SelectGame::new())
        .decorated(DecorateGame::new())
}

fn game_over<S: Storage, A: AudioPlayer>(
) -> impl EventRoutine<Return = (), Data = AppData<S, A>, View = AppView, Event = CommonEvent> {
    GameOverEventRoutine::new()
        .select(SelectGame::new())
        .decorated(DecorateGame::new())
}

fn win_text<S: Storage, A: AudioPlayer>() -> TextOverlay<S, A> {
    let bold = Style::new().with_foreground(Rgb24::new(255, 0, 0)).with_bold(true);
    let normal = Style::new().with_foreground(Rgb24::new_grey(255));
    let faint = Style::new().with_foreground(Rgb24::new_grey(127));
    TextOverlay::new(vec![
        text::RichTextPartOwned::new("The murky remains of the ".to_string(), normal),
        text::RichTextPartOwned::new("SOURCE OF SLIME".to_string(), bold),
        text::RichTextPartOwned::new(" drain into the stygian depths below. ".to_string(), normal),
        text::RichTextPartOwned::new("YOU HAVE WON.".to_string(), bold),
        text::RichTextPartOwned::new(" You emerge from the sewers into ".to_string(), normal),
        text::RichTextPartOwned::new("THE CITY ABOVE.".to_string(), bold),
        text::RichTextPartOwned::new("\n\nThe city which you saved. Repairs to a ".to_string(), normal),
        text::RichTextPartOwned::new("WAR-TORN WORLD".to_string(), bold),
        text::RichTextPartOwned::new(" are progressing smoothly, and a ".to_string(), normal),
        text::RichTextPartOwned::new("NEW MILLENNIUM".to_string(), bold),
        text::RichTextPartOwned::new(
            " is just around the corner. Things are finally looking up.".to_string(),
            normal,
        ),
        text::RichTextPartOwned::new("\n\nExcept for you. After all, what's a ".to_string(), normal),
        text::RichTextPartOwned::new("GENETICALLY-MODIFIED PRECOG SUPER-SOLDIER".to_string(), bold),
        text::RichTextPartOwned::new(
            " to do during peace time. You long for the day when more ".to_string(),
            normal,
        ),
        text::RichTextPartOwned::new("RADIOACTIVE MUTANT SLIMES".to_string(), bold),
        text::RichTextPartOwned::new(" appear in the sewers...".to_string(), normal),
        text::RichTextPartOwned::new("\n\n\n\n\n\nPress any key...".to_string(), faint),
    ])
}

fn win<S: Storage, A: AudioPlayer>(
) -> impl EventRoutine<Return = (), Data = AppData<S, A>, View = AppView, Event = CommonEvent> {
    SideEffectThen::new_with_view(|data: &mut AppData<S, A>, _: &_| {
        data.game.loop_music(Audio::EndText, 0.2);
        data.won = true;
        win_text()
    })
}

fn story<S: Storage, A: AudioPlayer>() -> TextOverlay<S, A> {
    let bold = Style::new().with_foreground(Rgb24::new(0, 255, 255)).with_bold(true);
    let normal = Style::new().with_foreground(Rgb24::new_grey(255));
    let faint = Style::new().with_foreground(Rgb24::new_grey(127));
    TextOverlay::new(vec![
        text::RichTextPartOwned::new("In the not-too-distant future, ".to_string(), normal),
        text::RichTextPartOwned::new("THE YEAR 1999,".to_string(), bold),
        text::RichTextPartOwned::new(" fallout from ".to_string(), normal),
        text::RichTextPartOwned::new("THE WAR".to_string(), bold),
        text::RichTextPartOwned::new(" has caused ".to_string(), normal),
        text::RichTextPartOwned::new("RADIOACTIVE MUTANT SLIMES".to_string(), bold),
        text::RichTextPartOwned::new(" to appear in the sewers of ".to_string(), normal),
        text::RichTextPartOwned::new("THE CITY.".to_string(), bold),
        text::RichTextPartOwned::new(" You are a ".to_string(), normal),
        text::RichTextPartOwned::new("GENETICALLY-MODIFIED PRECOG SUPER-SOLDIER,".to_string(), bold),
        text::RichTextPartOwned::new(
            " whose free-will was in-part traded for the power to ".to_string(),
            normal,
        ),
        text::RichTextPartOwned::new("PREDICT THE OUTCOME OF COMBAT ENCOUNTERS.".to_string(), bold),
        text::RichTextPartOwned::new(" Go into the sewers and ".to_string(), normal),
        text::RichTextPartOwned::new("ELIMINATE THE SOURCE OF SLIME!".to_string(), bold),
        text::RichTextPartOwned::new("\n\n\n\n\n\nPress any key...".to_string(), faint),
    ])
}

fn keybindings<S: Storage, A: AudioPlayer>() -> TextOverlay<S, A> {
    let normal = Style::new().with_foreground(Rgb24::new_grey(255));
    let faint = Style::new().with_foreground(Rgb24::new_grey(127));
    TextOverlay::new(vec![
        text::RichTextPartOwned::new("Movement/Aim: arrows/VI keys/WASD\n\n".to_string(), normal),
        text::RichTextPartOwned::new("Cancel Aim: escape\n\n".to_string(), normal),
        text::RichTextPartOwned::new("Wait: space\n\n".to_string(), normal),
        text::RichTextPartOwned::new("Use Tech: t\n\n".to_string(), normal),
        text::RichTextPartOwned::new("Examine: x\n\n".to_string(), normal),
        text::RichTextPartOwned::new("\n\n\n\n\nPress any key...".to_string(), faint),
    ])
}

fn aim<S: Storage, A: AudioPlayer>(
) -> impl EventRoutine<Return = Option<Coord>, Data = AppData<S, A>, View = AppView, Event = CommonEvent> {
    make_either!(Ei = A | B);
    SideEffectThen::new_with_view(|data: &mut AppData<S, A>, _view: &AppView| {
        let game_relative_mouse_coord = ScreenCoord(data.last_mouse_coord);
        if let Ok(initial_aim_coord) = data.game.initial_aim_coord(game_relative_mouse_coord) {
            Ei::A(
                AimEventRoutine::new(initial_aim_coord)
                    .select(SelectGame::new())
                    .decorated(DecorateGame::new()),
            )
        } else {
            Ei::B(Value::new(None))
        }
    })
}

fn examine<S: Storage, A: AudioPlayer>(
) -> impl EventRoutine<Return = (), Data = AppData<S, A>, View = AppView, Event = CommonEvent> {
    make_either!(Ei = A | B);
    SideEffectThen::new_with_view(|data: &mut AppData<S, A>, _view: &AppView| {
        let game_relative_mouse_coord = ScreenCoord(data.last_mouse_coord);
        if let Ok(initial_aim_coord) = data.game.initial_aim_coord(game_relative_mouse_coord) {
            Ei::A(
                ExamineEventRoutine::new(initial_aim_coord.0)
                    .select(SelectGame::new())
                    .decorated(DecorateGame::new()),
            )
        } else {
            Ei::B(Value::new(()))
        }
    })
}

enum GameLoopBreak {
    GameOver,
    Win,
    Pause,
}

fn game_loop<S: Storage, A: AudioPlayer>(
) -> impl EventRoutine<Return = (), Data = AppData<S, A>, View = AppView, Event = CommonEvent> {
    make_either!(Ei = A | B | C | D);
    SideEffect::new_with_view(|data: &mut AppData<S, A>, _: &_| data.game.pre_game_loop())
        .then(|| {
            Ei::A(game())
                .repeat(|game_return| match game_return {
                    GameReturn::LevelChange(ability_choice) => {
                        Handled::Continue(Ei::C(level_change_menu(ability_choice).and_then(|choice| {
                            make_either!(Ei = A | B);
                            match choice {
                                Err(menu::Escape) => Ei::A(Value::new(GameReturn::Pause)),
                                Ok(ability) => Ei::B(game_injecting_inputs(vec![InjectedInput::LevelChange(ability)])),
                            }
                        })))
                    }
                    GameReturn::Examine => Handled::Continue(Ei::D(examine().and_then(|()| game()))),
                    GameReturn::Pause => Handled::Return(GameLoopBreak::Pause),
                    GameReturn::GameOver => Handled::Return(GameLoopBreak::GameOver),
                    GameReturn::Win => Handled::Return(GameLoopBreak::Win),
                    GameReturn::Aim => Handled::Continue(Ei::B(aim().and_then(|maybe_coord| {
                        make_either!(Ei = A | B);
                        if let Some(coord) = maybe_coord {
                            Ei::A(game_injecting_inputs(vec![InjectedInput::Tech(coord)]))
                        } else {
                            Ei::B(game())
                        }
                    }))),
                })
                .and_then(|game_loop_break| {
                    make_either!(Ei = A | B | C);
                    match game_loop_break {
                        GameLoopBreak::Win => {
                            Ei::C(SideEffectThen::new_with_view(|data: &mut AppData<S, A>, _: &_| {
                                data.game.clear_instance();
                                win()
                            }))
                        }
                        GameLoopBreak::Pause => Ei::A(Value::new(())),
                        GameLoopBreak::GameOver => Ei::B(game_over().and_then(|()| {
                            SideEffect::new_with_view(|data: &mut AppData<S, A>, _: &_| {
                                data.game.clear_instance();
                            })
                        })),
                    }
                })
        })
        .then(|| SideEffect::new_with_view(|data: &mut AppData<S, A>, _: &_| data.game.post_game_loop()))
}

fn main_menu_cycle<S: Storage, A: AudioPlayer>(
    auto_play: Option<AutoPlay>,
    first_run: Option<FirstRun>,
) -> impl EventRoutine<Return = Option<Quit>, Data = AppData<S, A>, View = AppView, Event = CommonEvent> {
    make_either!(Ei = A | B | C | D | E | F | G | H | I | J);
    main_menu(auto_play, first_run).and_then(|entry| match entry {
        Ok(MainMenuEntry::Quit) => Ei::A(Value::new(Some(Quit))),
        Ok(MainMenuEntry::SaveQuit) => Ei::D(SideEffect::new_with_view(|data: &mut AppData<S, A>, _: &_| {
            data.game.save_instance();
            Some(Quit)
        })),
        Ok(MainMenuEntry::Save) => Ei::E(SideEffectThen::new_with_view(|data: &mut AppData<S, A>, _: &_| {
            make_either!(Ei = A | B);
            data.game.save_instance();
            if data.game.has_instance() {
                Ei::A(game_loop().map(|_| None))
            } else {
                Ei::B(Value::new(None))
            }
        })),
        Ok(MainMenuEntry::Clear) => Ei::F(SideEffect::new_with_view(|data: &mut AppData<S, A>, _: &_| {
            data.game.clear_instance();
            None
        })),
        Ok(MainMenuEntry::Resume) | Err(menu::Escape) => {
            Ei::B(SideEffectThen::new_with_view(|data: &mut AppData<S, A>, _: &_| {
                make_either!(Ei = A | B);
                if data.game.has_instance() {
                    Ei::A(game_loop().map(|()| None))
                } else {
                    Ei::B(Value::new(None))
                }
            }))
        }
        Ok(MainMenuEntry::NewGame) => Ei::C(SideEffectThen::new_with_view(|data: &mut AppData<S, A>, _: &_| {
            data.game.instantiate();
            data.main_menu.menu_instance_mut().set_index(0);
            game_loop().map(|()| None)
        })),
        Ok(MainMenuEntry::Options) => Ei::G(options_menu_cycle().map(|_| None)),
        Ok(MainMenuEntry::Story) => Ei::H(story().map(|()| None)),
        Ok(MainMenuEntry::Keybindings) => Ei::I(keybindings().map(|()| None)),
        Ok(MainMenuEntry::EndText) => Ei::J(win_text().map(|()| None)),
    })
}

fn event_routine<S: Storage, A: AudioPlayer>(
    initial_auto_play: Option<AutoPlay>,
) -> impl EventRoutine<Return = (), Data = AppData<S, A>, View = AppView, Event = CommonEvent> {
    MouseTracker::new(SideEffectThen::new_with_view(move |data: &mut AppData<S, A>, _: &_| {
        let mut config = data.game.config();
        let first_run = config.first_run;
        config.first_run = false;
        data.game.set_config(config);
        let first_run = if first_run { Some(FirstRun) } else { None };
        main_menu_cycle(initial_auto_play, first_run)
            .repeat(|maybe_quit| {
                if let Some(Quit) = maybe_quit {
                    Handled::Return(())
                } else {
                    Handled::Continue(main_menu_cycle(None, None))
                }
            })
            .return_on_exit(|data| {
                data.game.save_instance();
                ()
            })
    }))
}

pub trait Env {
    fn fullscreen(&self) -> bool;
    fn fullscreen_requires_restart(&self) -> bool;
    fn fullscreen_supported(&self) -> bool;
    // hack to get around fact that changing fullscreen mid-game on windows crashes
    fn set_fullscreen_init(&self, fullscreen: bool);
    fn set_fullscreen(&self, fullscreen: bool);
}
pub struct EnvNull;
impl Env for EnvNull {
    fn fullscreen(&self) -> bool {
        false
    }
    fn fullscreen_requires_restart(&self) -> bool {
        false
    }
    fn fullscreen_supported(&self) -> bool {
        false
    }
    fn set_fullscreen(&self, _fullscreen: bool) {}
    fn set_fullscreen_init(&self, _fullscreen: bool) {}
}

pub struct Fullscreen;

pub fn app<S: Storage, A: AudioPlayer>(
    game_config: GameConfig,
    frontend: Frontend,
    controls: Controls,
    storage: S,
    save_key: String,
    audio_player: A,
    rng_seed: RngSeed,
    auto_play: Option<AutoPlay>,
    fullscreen: Option<Fullscreen>,
    env: Box<dyn Env>,
) -> impl app::App {
    let app_data = AppData::new(
        game_config,
        frontend,
        controls,
        storage,
        save_key,
        audio_player,
        rng_seed,
        fullscreen,
        env,
    );
    let app_view = AppView::new();
    event_routine(auto_play).app_one_shot_ignore_return(app_data, app_view)
}
