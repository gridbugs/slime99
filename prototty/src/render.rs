use crate::{blink::Blink, depth, game::GameStatus, ui};
use direction::CardinalDirection;
use game::{ActionError, CellVisibility, Game, Layer, NpcAction, Tile, ToRenderEntity, MAP_SIZE};
use line_2d::{Config as LineConfig, LineSegment};
use prototty::render::{blend_mode, ColModify, Coord, Frame, Rgb24, Style, View, ViewCell, ViewContext};
use prototty::text::{wrap, StringView, StringViewSingleLine};
use std::time::Duration;

#[derive(Clone, Copy)]
pub enum Mode {
    Normal,
    Aim { blink_duration: Duration, target: Coord },
}

pub struct GameToRender<'a> {
    pub game: &'a Game,
    pub status: GameStatus,
    pub mouse_coord: Option<Coord>,
    pub mode: Mode,
    pub action_error: Option<ActionError>,
}

pub struct GameView {
    last_offset: Coord,
    blink: Blink,
}

#[derive(Clone, Copy, Debug)]
enum MessageVerb {
    See,
    Remember,
}

impl GameView {
    pub fn new() -> Self {
        Self {
            last_offset: Coord::new(0, 0),
            blink: Blink::new(),
        }
    }

    pub fn absolute_coord_to_game_relative_screen_coord(&self, coord: Coord) -> Coord {
        coord - self.last_offset
    }

    pub fn view<F: Frame, C: ColModify>(
        &mut self,
        game_to_render: GameToRender,
        context: ViewContext<C>,
        frame: &mut F,
    ) {
        match game_to_render.status {
            GameStatus::Playing => {
                let mut entity_under_cursor = None;
                for entity in game_to_render.game.to_render_entities() {
                    render_entity(&entity, game_to_render.game, context, frame);
                    if let Some(mouse_coord) = game_to_render.mouse_coord {
                        let game_coord = mouse_coord / 2;
                        if entity.coord == game_coord {
                            let verb = match game_to_render.game.visibility_grid().cell_visibility(entity.coord) {
                                CellVisibility::CurrentlyVisibleWithLightColour(Some(_)) => Some(MessageVerb::See),
                                CellVisibility::PreviouslyVisible => Some(MessageVerb::Remember),
                                CellVisibility::NeverVisible
                                | CellVisibility::CurrentlyVisibleWithLightColour(None) => None,
                            };
                            if let Some(verb) = verb {
                                if let Some((max_depth, _tile, _verb)) = entity_under_cursor {
                                    let depth = layer_depth(entity.layer);
                                    if depth > max_depth {
                                        entity_under_cursor = Some((depth, entity.tile, verb));
                                    }
                                } else {
                                    entity_under_cursor = Some((layer_depth(entity.layer), entity.tile, verb));
                                }
                            }
                        }
                    }
                }
                if let Some((_depth, tile, verb)) = entity_under_cursor {
                    if let Some(description) = tile_str(tile) {
                        let verb_str = match verb {
                            MessageVerb::Remember => "remember seeing",
                            MessageVerb::See => "see",
                        };
                        let mut buf = String::new();
                        use std::fmt::Write;
                        write!(&mut buf, "You {} {} here.", verb_str, description).unwrap();
                        StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(255))).view(
                            &buf,
                            context.add_offset(Coord::new(0, MAP_SIZE.height() as i32 * 2)),
                            frame,
                        );
                    }
                }
            }
            GameStatus::Over => {
                for entity in game_to_render.game.to_render_entities() {
                    render_entity_game_over(&entity, game_to_render.game, context, frame);
                }
                StringView::new(Style::new().with_foreground(Rgb24::new(255, 0, 0)), wrap::Word::new()).view(
                    "You failed. The slimes overrun the city and CONSUME WHAT REMAINS OF HUMANITY. Press a key to continue...",
                    context.add_offset(Coord::new(0, MAP_SIZE.height() as i32 * 2)),
                    frame,
                );
            }
        }
        if let Some(action_error) = game_to_render.action_error {
            let s = action_error_str(action_error);
            StringView::new(
                Style::new().with_foreground(Rgb24::new(255, 255, 255)),
                wrap::Word::new(),
            )
            .view(
                s,
                context.add_offset(Coord::new(0, MAP_SIZE.height() as i32 * 2 + 1)),
                frame,
            );
        }
        let ui = ui::Ui {
            player: game_to_render.game.player(),
        };
        ui::UiView.view(ui, context.add_offset(Coord::new(39, 0)), frame);
        match game_to_render.mode {
            Mode::Normal => (),
            Mode::Aim { blink_duration, target } => {
                let aim_coord = target / 2;
                let player_coord = game_to_render.game.player_coord();
                if aim_coord != player_coord {
                    for node in LineSegment::new(player_coord, aim_coord).config_node_iter(LineConfig {
                        exclude_start: true,
                        exclude_end: true,
                    }) {
                        if !node.coord.is_valid(game::MAP_SIZE) {
                            break;
                        }
                        for &offset in &quad::OFFSETS {
                            let output_coord = node.coord * 2 + offset;
                            frame.blend_cell_background_relative(
                                output_coord,
                                depth::GAME_MAX,
                                Rgb24::new(255, 0, 0),
                                127,
                                blend_mode::LinearInterpolate,
                                context,
                            );
                        }
                    }
                }
                if aim_coord.is_valid(game::MAP_SIZE) {
                    for &offset in &quad::OFFSETS {
                        let alpha = self.blink.alpha(blink_duration);
                        let output_coord = aim_coord * 2 + offset;
                        frame.blend_cell_background_relative(
                            output_coord,
                            depth::GAME_MAX,
                            Rgb24::new(255, 0, 0),
                            alpha,
                            blend_mode::LinearInterpolate,
                            context,
                        );
                    }
                }
            }
        }
    }
}

mod quad {
    use super::Coord;
    pub const OFFSETS: [Coord; 4] = [Coord::new(0, 0), Coord::new(1, 0), Coord::new(0, 1), Coord::new(1, 1)];
}

struct Quad {
    cells: [ViewCell; 4],
}

fn apply_lighting(cell_colour: Rgb24, light_colour: Rgb24) -> Rgb24 {
    let base_colour = cell_colour
        .saturating_add(light_colour.scalar_div(4))
        .saturating_sub(light_colour.complement().scalar_div(1));
    base_colour.normalised_mul(light_colour)
}

impl Quad {
    fn new_repeating(to_repeat: ViewCell) -> Self {
        Self {
            cells: [to_repeat, to_repeat, to_repeat, to_repeat],
        }
    }
    fn enumerate<'a>(&'a self) -> impl 'a + Iterator<Item = (Coord, ViewCell)> {
        quad::OFFSETS.iter().cloned().zip(self.cells.iter().cloned())
    }
    fn new_wall_front(front_col: Rgb24, top_col: Rgb24) -> Self {
        let front = ViewCell::new().with_character(' ').with_background(front_col);
        let top = ViewCell::new()
            .with_character('█')
            .with_background(front_col)
            .with_foreground(top_col);
        Self {
            cells: [top, top, front, front],
        }
    }
    fn new_wall_top(top: Rgb24) -> Self {
        let top = ViewCell::new().with_character(' ').with_background(top);
        Self::new_repeating(top)
    }
    fn new_floor(foreground: Rgb24, background: Rgb24) -> Self {
        let base = ViewCell::new().with_foreground(foreground).with_background(background);
        Self {
            cells: [
                base.with_character('▗'),
                base.with_character('▖'),
                base.with_character('▝'),
                base.with_character('▘'),
            ],
        }
    }
    fn new_door_closed(foreground: Rgb24, background: Rgb24) -> Self {
        let base = ViewCell::new().with_foreground(background).with_background(foreground);
        Self {
            cells: [
                base.with_character('▘'),
                base.with_character('▝'),
                base.with_character('▖'),
                base.with_character('▗'),
            ],
        }
    }
    fn new_door_open(foreground: Rgb24, background: Rgb24) -> Self {
        let base = ViewCell::new().with_foreground(foreground).with_background(background);
        Self {
            cells: [
                base.with_character('▄'),
                base.with_character('▄'),
                base.with_character('▀'),
                base.with_character('▀'),
            ],
        }
    }
    fn new_stairs(foreground: Rgb24, background: Rgb24) -> Self {
        let base = ViewCell::new().with_bold(true);
        Self {
            cells: [
                base.with_character('▝')
                    .with_foreground(background)
                    .with_background(foreground),
                base.with_character(' ').with_background(background),
                base.with_character(' ').with_background(foreground),
                base.with_character('▝')
                    .with_foreground(background)
                    .with_background(foreground),
            ],
        }
    }
    fn new_player(foreground: Rgb24) -> Self {
        let base = ViewCell::new().with_bold(true).with_foreground(foreground);
        Self {
            cells: [
                base.with_character('╔'),
                base.with_character('╗'),
                base.with_character('╚'),
                base.with_character('╩'),
            ],
        }
    }
    fn new_slime(
        character: char,
        foreground: Rgb24,
        background: Rgb24,
        hit_points: u32,
        next_action: NpcAction,
    ) -> Self {
        let base = ViewCell::new().with_background(background).with_foreground(foreground);
        let action_character = match next_action {
            NpcAction::Wait => ' ',
            NpcAction::Walk(direction) => match direction {
                CardinalDirection::North => '↑',
                CardinalDirection::East => '→',
                CardinalDirection::South => '↓',
                CardinalDirection::West => '←',
            },
        };
        Self {
            cells: [
                base.with_character(character)
                    .with_bold(true)
                    .with_foreground(foreground),
                base.with_character(action_character),
                base.with_character(std::char::from_digit((hit_points / 10) % 10, 10).unwrap()),
                base.with_character(std::char::from_digit(hit_points % 10, 10).unwrap()),
            ],
        }
    }
    fn apply_lighting(&mut self, light_colour: Rgb24) {
        for view_cell in self.cells.iter_mut() {
            if let Some(foreground) = view_cell.style.foreground.as_mut() {
                *foreground = apply_lighting(*foreground, light_colour);
            }
            if let Some(background) = view_cell.style.background.as_mut() {
                *background = apply_lighting(*background, light_colour);
            }
        }
    }
}

fn entity_to_quad_visible(entity: &ToRenderEntity, game: &Game) -> Quad {
    match entity.tile {
        Tile::Player => Quad::new_player(Rgb24::new(255, 255, 255)),
        Tile::Floor => Quad::new_floor(Rgb24::new(0, 187, 187), Rgb24::new(0, 127, 127)),
        Tile::Wall => {
            let below = entity.coord + Coord::new(0, 1);
            if game.contains_wall(below) && !game.visibility_grid().is_coord_never_visible(below) {
                Quad::new_wall_top(Rgb24::new(255, 0, 255))
            } else {
                Quad::new_wall_front(Rgb24::new(127, 0, 127), Rgb24::new(255, 0, 255))
            }
        }
        Tile::DoorClosed => Quad::new_door_closed(Rgb24::new(255, 127, 255), Rgb24::new(127, 0, 127)),
        Tile::DoorOpen => Quad::new_door_open(Rgb24::new(255, 127, 255), Rgb24::new(0, 127, 127)),
        Tile::Stairs => Quad::new_stairs(Rgb24::new(255, 255, 255), Rgb24::new(0, 127, 127)),
        Tile::Sludge0 => {
            let background = entity.colour_hint.unwrap_or_else(|| Rgb24::new(255, 0, 0));
            let foreground = background.scalar_div(2);
            Quad::new_repeating(
                ViewCell::new()
                    .with_character('~')
                    .with_foreground(foreground)
                    .with_background(background),
            )
        }
        Tile::Sludge1 => {
            let background = entity.colour_hint.unwrap_or_else(|| Rgb24::new(255, 0, 0));
            let foreground = background.scalar_div(2);
            Quad::new_repeating(
                ViewCell::new()
                    .with_character('≈')
                    .with_foreground(foreground)
                    .with_background(background),
            )
        }
        Tile::Bridge => {
            let character = if game.contains_bridge(entity.coord + Coord::new(0, 1))
                || game.contains_bridge(entity.coord - Coord::new(0, 1))
            {
                '║'
            } else {
                '═'
            };
            Quad::new_repeating(
                ViewCell::new()
                    .with_character(character)
                    .with_foreground(Rgb24::new(127, 127, 0))
                    .with_background(Rgb24::new(200, 127, 0)),
            )
        }
        Tile::SlimeDivide => Quad::new_slime(
            'd',
            Rgb24::new(255, 63, 63),
            Rgb24::new(31, 15, 15),
            entity.hit_points.map(|hp| hp.current).unwrap_or(0),
            entity.next_action.unwrap_or(NpcAction::Wait),
        ),
        Tile::SlimeSwap => Quad::new_slime(
            's',
            Rgb24::new(127, 127, 255),
            Rgb24::new(15, 15, 31),
            entity.hit_points.map(|hp| hp.current).unwrap_or(0),
            entity.next_action.unwrap_or(NpcAction::Wait),
        ),
        Tile::SlimeTeleport => Quad::new_slime(
            't',
            Rgb24::new(187, 63, 255),
            Rgb24::new(15, 0, 31),
            entity.hit_points.map(|hp| hp.current).unwrap_or(0),
            entity.next_action.unwrap_or(NpcAction::Wait),
        ),
        Tile::SlimeGoo => Quad::new_slime(
            'g',
            Rgb24::new(0, 255, 0),
            Rgb24::new(0, 63, 0),
            entity.hit_points.map(|hp| hp.current).unwrap_or(0),
            entity.next_action.unwrap_or(NpcAction::Wait),
        ),
        Tile::SlimeAttackUpgrade => Quad::new_slime(
            'A',
            Rgb24::new(255, 255, 255),
            Rgb24::new(31, 31, 31),
            entity.hit_points.map(|hp| hp.current).unwrap_or(0),
            entity.next_action.unwrap_or(NpcAction::Wait),
        ),
        Tile::SlimeDefendUpgrade => Quad::new_slime(
            'D',
            Rgb24::new(255, 255, 255),
            Rgb24::new(31, 31, 31),
            entity.hit_points.map(|hp| hp.current).unwrap_or(0),
            entity.next_action.unwrap_or(NpcAction::Wait),
        ),
        Tile::SlimeTechUpgrade => Quad::new_slime(
            'T',
            Rgb24::new(255, 255, 255),
            Rgb24::new(31, 31, 31),
            entity.hit_points.map(|hp| hp.current).unwrap_or(0),
            entity.next_action.unwrap_or(NpcAction::Wait),
        ),

        Tile::SlimeCurse => Quad::new_slime(
            'c',
            Rgb24::new(187, 187, 187),
            Rgb24::new(31, 31, 31),
            entity.hit_points.map(|hp| hp.current).unwrap_or(0),
            entity.next_action.unwrap_or(NpcAction::Wait),
        ),
    }
}

fn entity_to_quad_remembered(entity: &ToRenderEntity, game: &Game) -> Option<Quad> {
    let foreground = Rgb24::new_grey(63);
    let background = Rgb24::new_grey(15);
    let quad = match entity.tile {
        Tile::Floor => Quad::new_floor(foreground, background),
        Tile::Wall => {
            if game.contains_wall(entity.coord + Coord::new(0, 1)) {
                Quad::new_wall_top(foreground)
            } else {
                Quad::new_wall_front(background, foreground)
            }
        }
        Tile::DoorClosed => Quad::new_door_closed(foreground, background),
        Tile::DoorOpen => Quad::new_door_closed(foreground, background),
        Tile::Stairs => Quad::new_stairs(foreground, background),
        Tile::Sludge0 | Tile::Sludge1 => Quad::new_repeating(
            ViewCell::new()
                .with_character('~')
                .with_foreground(foreground)
                .with_background(background),
        ),
        Tile::Bridge => {
            let character = if game.contains_bridge(entity.coord + Coord::new(0, 1))
                || game.contains_bridge(entity.coord - Coord::new(0, 1))
            {
                '║'
            } else {
                '═'
            };
            Quad::new_repeating(
                ViewCell::new()
                    .with_character(character)
                    .with_foreground(foreground)
                    .with_background(background),
            )
        }
        _ => return None,
    };
    Some(quad)
}

fn layer_depth(layer: Option<Layer>) -> i8 {
    if let Some(layer) = layer {
        match layer {
            Layer::Floor => 0,
            Layer::Feature => 1,
            Layer::Character => 2,
        }
    } else {
        depth::GAME_MAX - 1
    }
}

fn render_quad<F: Frame, C: ColModify>(coord: Coord, depth: i8, quad: &Quad, context: ViewContext<C>, frame: &mut F) {
    for (offset, view_cell) in quad.enumerate() {
        let output_coord = coord * 2 + offset;
        frame.set_cell_relative(output_coord, depth, view_cell, context);
    }
}

fn render_entity<F: Frame, C: ColModify>(entity: &ToRenderEntity, game: &Game, context: ViewContext<C>, frame: &mut F) {
    match game.visibility_grid().cell_visibility(entity.coord) {
        CellVisibility::CurrentlyVisibleWithLightColour(Some(light_colour)) => {
            let mut quad = entity_to_quad_visible(entity, game);
            let depth = layer_depth(entity.layer);
            quad.apply_lighting(light_colour);
            render_quad(entity.coord, depth, &quad, context, frame);
        }
        CellVisibility::PreviouslyVisible => {
            if let Some(quad) = entity_to_quad_remembered(entity, game) {
                let depth = layer_depth(entity.layer);
                render_quad(entity.coord, depth, &quad, context, frame);
            }
        }
        CellVisibility::NeverVisible | CellVisibility::CurrentlyVisibleWithLightColour(None) => (),
    }
}

fn render_entity_game_over<F: Frame, C: ColModify>(
    entity: &ToRenderEntity,
    game: &Game,
    context: ViewContext<C>,
    frame: &mut F,
) {
    let mut quad = entity_to_quad_visible(entity, game);
    let depth = layer_depth(entity.layer);
    quad.apply_lighting(Rgb24::new(255, 87, 31));
    render_quad(entity.coord, depth, &quad, context, frame);
}

fn tile_str(tile: Tile) -> Option<&'static str> {
    match tile {
        Tile::Player => Some("yourself"),
        Tile::Sludge0 | Tile::Sludge1 => Some("some toxic sludge"),
        Tile::Bridge => Some("a bridge over toxic sludge"),
        Tile::DoorClosed | Tile::DoorOpen => Some("a door"),
        Tile::Wall => Some("a wall"),
        Tile::Floor => Some("the floor"),
        Tile::Stairs => Some("a staircase leading further down"),
        Tile::SlimeDivide => Some("a Divide Slime"),
        Tile::SlimeSwap => Some("a Swap Slime"),
        Tile::SlimeTeleport => Some("a Teleport Slime"),
        Tile::SlimeGoo => Some("a Goo Slime"),
        Tile::SlimeAttackUpgrade => Some("an Attack Upgrade Slime"),
        Tile::SlimeDefendUpgrade => Some("a Defend Upgrade Slime"),
        Tile::SlimeTechUpgrade => Some("a Tech Upgrade Slime"),
        Tile::SlimeCurse => Some("a Curse Slime"),
    }
}

fn action_error_str(action_error: ActionError) -> &'static str {
    match action_error {
        ActionError::BlinkToNonVisibleCell => "Can't blink to non-visible location",
        ActionError::BlinkToSolidCell => "Can't blink to solid cell",
        ActionError::NoTechToApply => "Tech stack is empty",
        ActionError::BlinkWithoutDestination => "Can't blink without destination",
        ActionError::AttackDeckFull => "Attack stack is full",
        ActionError::DefendDeckFull => "Defend stack is full",
        ActionError::WalkIntoSolidCell => "You can't walk there",
        ActionError::NoAbilityInSlot => "No ability in slot",
        ActionError::NotEnoughAttacks => "Too few attacks",
        ActionError::NotEnoughDefends => "Too few defends",
        ActionError::NotEnoughTechs => "Too few techs",
    }
}
