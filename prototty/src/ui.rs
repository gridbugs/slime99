use game::player::{Ability, AbilityTable, AbilityTarget, Attack, Deck, Defend, Player, Tech, EMPTY_ATTACK};
use prototty::render::{ColModify, Coord, Frame, Rgb24, Style, View, ViewContext};
use prototty::text::StringViewSingleLine;

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
        Defend::Revenge => write!(s, "Revenge").unwrap(),
        Defend::Armour(n) => write!(s, "Armour {}", n).unwrap(),
        Defend::SkipAttack => write!(s, "Skip Attack").unwrap(),
    }
}

fn write_tech(tech: Tech, s: &mut String) {
    use std::fmt::Write;
    match tech {
        Tech::Blink => write!(s, "Blink").unwrap(),
        Tech::CritNext => write!(s, "Crit Next").unwrap(),
        Tech::Attract => write!(s, "Attract").unwrap(),
        Tech::Repel => write!(s, "Repel").unwrap(),
        Tech::MissNext => write!(s, "Miss Next").unwrap(),
        Tech::TeleportNext => write!(s, "Teleport Next").unwrap(),
        Tech::Skip => write!(s, "Skip").unwrap(),
    }
}

fn write_ability_target(ability_target: AbilityTarget, s: &mut String) {
    use std::fmt::Write;
    match ability_target {
        AbilityTarget::Attack => write!(s, "Atk").unwrap(),
        AbilityTarget::Defend => write!(s, "Def").unwrap(),
        AbilityTarget::Tech => write!(s, "Tch").unwrap(),
    }
}
pub fn write_abiilty(abiilty: Ability, s: &mut String) {
    use std::fmt::Write;
    match abiilty {
        Ability::Stash(target) => {
            write!(s, "Stash ").unwrap();
            write_ability_target(target, s);
        }
        Ability::SwapTop2(target) => {
            write!(s, "Swap top 2 ").unwrap();
            write_ability_target(target, s);
        }
        Ability::Discard(target) => {
            write!(s, "Discard ").unwrap();
            write_ability_target(target, s);
        }
    }
}

fn view_attack_list<F: Frame, C: ColModify>(attack: &Deck<Attack>, context: ViewContext<C>, frame: &mut F) {
    StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(255))).view("Atk:", context, frame);
    let padding = attack.max_size() - attack.len();
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
    let empty_colour = if attack.len() == 0 {
        Rgb24::new(255, 0, 0)
    } else {
        Rgb24::new_grey(63)
    };
    buf.clear();
    write_attack(EMPTY_ATTACK, &mut buf);
    StringViewSingleLine::new(Style::new().with_foreground(empty_colour)).view(
        &buf,
        context.add_offset(Coord::new(0, attack.max_size() as i32 + 1)),
        frame,
    );
}
fn view_defend_list<F: Frame, C: ColModify>(defend: &Deck<Defend>, context: ViewContext<C>, frame: &mut F) {
    StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(255))).view("Def:", context, frame);
    let padding = defend.max_size() - defend.len();
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
    let die_colour = if defend.len() == 0 {
        Rgb24::new(255, 0, 0)
    } else {
        Rgb24::new_grey(63)
    };
    StringViewSingleLine::new(Style::new().with_foreground(die_colour)).view(
        "Die",
        context.add_offset(Coord::new(0, defend.max_size() as i32 + 1)),
        frame,
    );
}
fn view_tech_list<F: Frame, C: ColModify>(tech: &Deck<Tech>, context: ViewContext<C>, frame: &mut F) {
    StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(255))).view("(t) Tch:", context, frame);
    let padding = tech.max_size() - tech.len();
    for i in 0..padding {
        StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(63))).view(
            "--",
            context.add_offset(Coord::new(0, i as i32 + 1)),
            frame,
        );
    }
    let mut buf = String::new();
    for (i, &tech) in tech.iter().enumerate() {
        let mut view = if i == 0 {
            StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(255)))
        } else {
            StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(127)))
        };
        buf.clear();
        write_tech(tech, &mut buf);
        view.view(&buf, context.add_offset(Coord::new(0, (i + padding) as i32 + 1)), frame);
    }
}
fn view_abiilty_list<F: Frame, C: ColModify>(ability: &AbilityTable, context: ViewContext<C>, frame: &mut F) {
    use std::fmt::Write;
    let mut buf = String::new();
    for (i, &abiilty) in ability.iter().enumerate() {
        buf.clear();
        write!(&mut buf, "({}) ", i + 1).unwrap();
        write_abiilty(abiilty, &mut buf);
        StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(255))).view(
            &buf,
            context.add_offset(Coord::new(0, i as i32)),
            frame,
        );
    }
    for i in 0..(ability.max_size() - ability.len()) {
        buf.clear();
        write!(&mut buf, "({}) --", i + 1 + ability.len()).unwrap();
        StringViewSingleLine::new(Style::new().with_foreground(Rgb24::new_grey(63))).view(
            &buf,
            context.add_offset(Coord::new(0, (ability.len() + i) as i32)),
            frame,
        );
    }
}

pub struct Ui<'a> {
    pub player: &'a Player,
}

pub struct UiView;

impl UiView {
    pub fn view<F: Frame, C: ColModify>(&mut self, ui: Ui, context: ViewContext<C>, frame: &mut F) {
        view_attack_list(&ui.player.attack, context, frame);
        view_defend_list(&ui.player.defend, context.add_offset(Coord::new(11, 0)), frame);
        view_tech_list(
            &ui.player.tech,
            context.add_offset(Coord::new(
                0,
                ui.player.attack.max_size().max(ui.player.defend.max_size()) as i32 + 3,
            )),
            frame,
        );
        view_abiilty_list(
            &ui.player.ability,
            context.add_offset(Coord::new(
                0,
                (ui.player.attack.max_size().max(ui.player.defend.max_size()) + ui.player.tech.max_size()) as i32 + 6,
            )),
            frame,
        );
    }
}
