use prototty::render::{ColModify, Coord, Frame, Rgb24, Style, View, ViewContext};
use prototty::text::StringViewSingleLine;

#[derive(Debug, Clone, Copy)]
enum Attack {
    Hit(u32),
    Cleave(u32),
    Skewer(u32),
    Miss,
}

#[derive(Debug, Clone, Copy)]
enum Defend {
    Dodge,
    Teleport,
    Panic,
}

#[derive(Debug, Clone, Copy)]
enum Ability {
    Blink,
    CritNext,
    Attract,
    Repel,
    MissNext,
    TeleportNext,
}

#[derive(Debug, Clone, Copy)]
enum Deck {
    Attack,
    Defend,
    Ability,
}

#[derive(Debug, Clone, Copy)]
enum MetaAbility {
    Stash(Deck),
    Skip(Deck),
}

fn write_attack(attack: Attack, s: &mut String) {
    use std::fmt::Write;
    match attack {
        Attack::Hit(n) => write!(s, "Hit {}", n).unwrap(),
        Attack::Cleave(n) => write!(s, "Cleave {}", n).unwrap(),
        Attack::Skewer(n) => write!(s, "Skewer {}", n).unwrap(),
        Attack::Miss => write!(s, "Miss").unwrap(),
    }
}

fn write_defend(defend: Defend, s: &mut String) {
    use std::fmt::Write;
    match defend {
        Defend::Dodge => write!(s, "Dodge").unwrap(),
        Defend::Teleport => write!(s, "Teleport").unwrap(),
        Defend::Panic => write!(s, "Panic").unwrap(),
    }
}

fn write_ability(ability: Ability, s: &mut String) {
    use std::fmt::Write;
    match ability {
        Ability::Blink => write!(s, "Blink").unwrap(),
        Ability::CritNext => write!(s, "Crit Next").unwrap(),
        Ability::Attract => write!(s, "Attract").unwrap(),
        Ability::Repel => write!(s, "Repel").unwrap(),
        Ability::MissNext => write!(s, "Miss Next").unwrap(),
        Ability::TeleportNext => write!(s, "Teleport Next").unwrap(),
    }
}

fn write_deck(deck: Deck, s: &mut String) {
    use std::fmt::Write;
    match deck {
        Deck::Attack => write!(s, "Atk").unwrap(),
        Deck::Defend => write!(s, "Def").unwrap(),
        Deck::Ability => write!(s, "Tech").unwrap(),
    }
}
fn write_meta_ability(meta_ability: MetaAbility, s: &mut String) {
    use std::fmt::Write;
    match meta_ability {
        MetaAbility::Stash(deck) => {
            write!(s, "Stash ").unwrap();
            write_deck(deck, s);
        }
        MetaAbility::Skip(deck) => {
            write!(s, "Skip ").unwrap();
            write_deck(deck, s);
        }
    }
}

fn view_attack_list<F: Frame, C: ColModify>(attack: &[Attack], context: ViewContext<C>, frame: &mut F) {
    StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(255))).view("Atk:", context, frame);
    let padding = MAX_ATTACK - attack.len();
    for i in 0..padding {
        StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(63))).view(
            "--",
            context.add_offset(Coord::new(0, i as i32 + 1)),
            frame,
        );
    }
    let mut buf = String::new();
    for (i, &attack) in attack.iter().enumerate() {
        let mut view = if i == 0 {
            StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(255)))
        } else {
            StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(127)))
        };
        buf.clear();
        write_attack(attack, &mut buf);
        view.view(&buf, context.add_offset(Coord::new(0, (i + padding) as i32 + 1)), frame);
    }
}
fn view_defend_list<F: Frame, C: ColModify>(defend: &[Defend], context: ViewContext<C>, frame: &mut F) {
    StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(255))).view("Def:", context, frame);
    let padding = MAX_DEFEND - defend.len();
    for i in 0..padding {
        StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(63))).view(
            "--",
            context.add_offset(Coord::new(0, i as i32 + 1)),
            frame,
        );
    }
    let mut buf = String::new();
    for (i, &defend) in defend.iter().enumerate() {
        let mut view = if i == 0 {
            StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(255)))
        } else {
            StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(127)))
        };
        buf.clear();
        write_defend(defend, &mut buf);
        view.view(&buf, context.add_offset(Coord::new(0, (i + padding) as i32 + 1)), frame);
    }
    StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(63))).view(
        "Die",
        context.add_offset(Coord::new(0, MAX_DEFEND as i32 + 1)),
        frame,
    );
}
fn view_ability_list<F: Frame, C: ColModify>(ability: &[Ability], context: ViewContext<C>, frame: &mut F) {
    StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(255))).view("(t) Tech:", context, frame);
    let padding = MAX_ABILITY - ability.len();
    for i in 0..padding {
        StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(63))).view(
            "--",
            context.add_offset(Coord::new(0, i as i32 + 1)),
            frame,
        );
    }
    let mut buf = String::new();
    for (i, &ability) in ability.iter().enumerate() {
        let mut view = if i == 0 {
            StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(255)))
        } else {
            StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(127)))
        };
        buf.clear();
        write_ability(ability, &mut buf);
        view.view(&buf, context.add_offset(Coord::new(0, (i + padding) as i32 + 1)), frame);
    }
}
fn view_meta_ability_list<F: Frame, C: ColModify>(
    meta_ability: &[MetaAbility],
    context: ViewContext<C>,
    frame: &mut F,
) {
    use std::fmt::Write;
    let mut buf = String::new();
    for (i, &meta_ability) in meta_ability.iter().enumerate() {
        buf.clear();
        write!(&mut buf, "({}) ", i + 1).unwrap();
        write_meta_ability(meta_ability, &mut buf);
        StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(255))).view(
            &buf,
            context.add_offset(Coord::new(0, i as i32)),
            frame,
        );
    }
    for i in 0..(MAX_META_ABILITY - meta_ability.len()) {
        buf.clear();
        write!(&mut buf, "({}) --", i + 1 + meta_ability.len()).unwrap();
        StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(63))).view(
            &buf,
            context.add_offset(Coord::new(0, (meta_ability.len() + i) as i32)),
            frame,
        );
    }
}

const MAX_ATTACK: usize = 16;
const MAX_DEFEND: usize = 16;
const MAX_ABILITY: usize = 8;
const MAX_META_ABILITY: usize = 8;

pub struct Ui {
    attack: Vec<Attack>,
    defend: Vec<Defend>,
    ability: Vec<Ability>,
    meta_ability: Vec<MetaAbility>,
}

impl Ui {
    pub fn example() -> Self {
        Self {
            attack: vec![
                Attack::Hit(4),
                Attack::Miss,
                Attack::Hit(4),
                Attack::Skewer(4),
                Attack::Miss,
                Attack::Hit(4),
                Attack::Miss,
                Attack::Cleave(4),
                Attack::Miss,
                Attack::Cleave(4),
                Attack::Hit(100),
                Attack::Skewer(4),
            ],
            defend: vec![
                Defend::Dodge,
                Defend::Dodge,
                Defend::Dodge,
                Defend::Teleport,
                Defend::Panic,
                Defend::Dodge,
                Defend::Panic,
                Defend::Dodge,
                Defend::Dodge,
                Defend::Teleport,
                Defend::Dodge,
            ],
            ability: vec![
                Ability::Blink,
                Ability::MissNext,
                Ability::Attract,
                Ability::Blink,
                Ability::Repel,
                Ability::CritNext,
                Ability::TeleportNext,
            ],
            meta_ability: vec![
                MetaAbility::Skip(Deck::Attack),
                MetaAbility::Stash(Deck::Defend),
                MetaAbility::Stash(Deck::Attack),
                MetaAbility::Skip(Deck::Attack),
                MetaAbility::Stash(Deck::Ability),
            ],
        }
    }
}

pub struct UiView;

impl UiView {
    pub fn view<F: Frame, C: ColModify>(&mut self, ui: Ui, context: ViewContext<C>, frame: &mut F) {
        view_attack_list(&ui.attack, context, frame);
        view_defend_list(&ui.defend, context.add_offset(Coord::new(11, 0)), frame);
        view_ability_list(
            &ui.ability,
            context.add_offset(Coord::new(0, MAX_ATTACK.max(MAX_DEFEND) as i32 + 3)),
            frame,
        );
        view_meta_ability_list(
            &ui.meta_ability,
            context.add_offset(Coord::new(0, (MAX_ATTACK.max(MAX_DEFEND) + MAX_ABILITY) as i32 + 6)),
            frame,
        );
    }
}
