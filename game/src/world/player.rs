use crate::world::data::Item;
use rand::{seq::SliceRandom, Rng};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Attack {
    Hit(u32),
    Cleave(u32),
    Skewer(u32),
    Miss,
}

pub const EMPTY_ATTACK: Attack = Attack::Hit(4);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Defend {
    Armour(u32),
    Dodge,
    Teleport,
    Revenge,
    SkipAttack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tech {
    Blink,
    CritNext,
    Attract,
    Repel,
    MissNext,
    TeleportNext,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AbilityTarget {
    Attack,
    Defend,
    Tech,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ability {
    Stash(AbilityTarget),
    SwapTop2(AbilityTarget),
}

impl Ability {
    pub fn all() -> &'static [Ability] {
        &[
            Ability::Stash(AbilityTarget::Attack),
            Ability::Stash(AbilityTarget::Defend),
            Ability::Stash(AbilityTarget::Tech),
            Ability::SwapTop2(AbilityTarget::Attack),
            Ability::SwapTop2(AbilityTarget::Defend),
            Ability::SwapTop2(AbilityTarget::Tech),
        ]
    }
}

impl Tech {
    pub fn requires_aim(self) -> bool {
        use Tech::*;
        match self {
            Blink => true,
            CritNext => false,
            Attract => false,
            Repel => false,
            MissNext => false,
            TeleportNext => false,
            Skip => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deck<T> {
    items: Vec<T>,
    max_size: usize,
}

pub struct DeckIsFull;
pub struct NotEnoughCards;

impl<T> Deck<T> {
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter().rev()
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_full(&self) -> bool {
        self.len() == self.max_size
    }
    pub const fn max_size(&self) -> usize {
        self.max_size
    }
    pub fn pop(&mut self) -> Option<T> {
        self.items.pop()
    }
    pub fn peek(&self) -> Option<&T> {
        self.items.last()
    }
    pub fn push(&mut self, item: T) -> Result<(), DeckIsFull> {
        if self.items.len() < self.max_size {
            self.items.push(item);
            Ok(())
        } else {
            Err(DeckIsFull)
        }
    }
    pub fn swap_top_2(&mut self) -> Result<(), NotEnoughCards> {
        if self.items.len() < 2 {
            return Err(NotEnoughCards);
        }
        let a = self.items.len() - 1;
        let b = self.items.len() - 2;
        self.items.swap(a, b);
        Ok(())
    }
    pub fn stash(&mut self) -> Result<(), NotEnoughCards> {
        if self.items.len() < 2 {
            return Err(NotEnoughCards);
        }
        let top = self.items.pop().unwrap();
        self.items.insert(0, top);
        Ok(())
    }
    pub fn insert_random<R: Rng>(&mut self, item: T, rng: &mut R) -> Result<(), DeckIsFull> {
        if self.items.len() == self.max_size {
            return Err(DeckIsFull);
        }
        let index = rng.gen_range(0, self.items.len() + 1);
        self.items.insert(index, item);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilityTable {
    abilities: Vec<Ability>,
    max_size: usize,
}

impl AbilityTable {
    pub fn iter(&self) -> impl Iterator<Item = &Ability> {
        self.abilities.iter()
    }
    pub fn len(&self) -> usize {
        self.abilities.len()
    }
    pub const fn max_size(&self) -> usize {
        self.max_size
    }
    pub fn get(&self, index: usize) -> Option<Ability> {
        self.abilities.get(index).cloned()
    }
    pub fn push(&mut self, item: Ability) -> Result<(), DeckIsFull> {
        if self.abilities.len() < self.max_size {
            self.abilities.push(item);
            Ok(())
        } else {
            Err(DeckIsFull)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    pub attack: Deck<Attack>,
    pub defend: Deck<Defend>,
    pub tech: Deck<Tech>,
    pub ability: AbilityTable,
}

fn rev<T>(mut vec: Vec<T>) -> Vec<T> {
    vec.reverse();
    vec
}

fn shuf<T, R: Rng>(mut vec: Vec<T>, rng: &mut R) -> Vec<T> {
    vec.shuffle(rng);
    vec
}

impl Player {
    pub fn new<R: Rng>(rng: &mut R) -> Self {
        use Ability::*;
        use Attack::*;
        use Defend::*;
        use Tech::*;
        Self {
            attack: Deck {
                #[rustfmt::skip]
                items: rev(vec![
                    Hit(rng.gen_range(4, 10)),
                    Hit(rng.gen_range(4, 10)),
                    Hit(rng.gen_range(4, 10)),
                    Cleave(rng.gen_range(4, 10)),
                    Hit(rng.gen_range(8, 20)),
                    Hit(rng.gen_range(8, 20)),
                    Hit(rng.gen_range(12, 30)),
                    Hit(rng.gen_range(12, 30)),
                ]),
                max_size: 16,
            },
            defend: Deck {
                #[rustfmt::skip]
                items: rev(vec![
                    Armour(rng.gen_range(1, 2)),
                    Armour(rng.gen_range(1, 2)),
                    Armour(rng.gen_range(1, 2)),
                    Dodge,
                    Armour(rng.gen_range(1, 3)),
                    Armour(rng.gen_range(1, 3)),
                    Armour(rng.gen_range(1, 3)),
                    Teleport,
                    Armour(rng.gen_range(2, 5)),
                    Armour(rng.gen_range(2, 5)),
                ]),
                max_size: 16,
            },
            tech: Deck {
                #[rustfmt::skip]
                items: shuf(vec![
                   Attract,
                    Repel,
                    Repel,
                    Blink,
                    Blink,
                    Blink,
                ], rng),
                max_size: 8,
            },
            ability: AbilityTable {
                #[rustfmt::skip]
                abilities: vec![
                    Stash(AbilityTarget::Attack),
                    Stash(AbilityTarget::Defend),
                ],
                max_size: 8,
            },
        }
    }
}

#[derive(Clone, Copy)]
pub enum Outcome {
    Attack(Attack),
    Defend(Defend),
    Tech(Tech),
}

pub fn choose_attack<R: Rng>(level: u32, special: bool, rng: &mut R) -> Attack {
    if special {
        match rng.gen_range(0, 3) {
            0 => Attack::Hit(99),
            1 => Attack::Cleave(rng.gen_range((level + 1) * 6, (level + 1) * 9)),
            2 => Attack::Skewer(rng.gen_range((level + 1) * 6, (level + 1) * 9)),
            _ => unreachable!(),
        }
    } else {
        match rng.gen_range(0, 3) {
            0 => Attack::Hit(rng.gen_range((level + 1) * 4, (level + 1) * 7)),
            1 => Attack::Cleave(rng.gen_range((level + 1) * 3, (level + 1) * 6)),
            2 => Attack::Skewer(rng.gen_range((level + 1) * 3, (level + 1) * 6)),
            _ => unreachable!(),
        }
    }
}

pub fn choose_defend<R: Rng>(level: u32, special: bool, rng: &mut R) -> Defend {
    if special {
        match rng.gen_range(0, 2) {
            0 => Defend::Revenge,
            1 => Defend::Armour(rng.gen_range((level + 1) * 2, (level + 1) * 3)),
            _ => unreachable!(),
        }
    } else {
        match rng.gen_range(0, 4) {
            0 => Defend::Teleport,
            1 => Defend::Dodge,
            2 => Defend::Armour(level + 1),
            3 => Defend::Armour(rng.gen_range(level + 1, (level + 1) * 2)),
            _ => unreachable!(),
        }
    }
}

pub fn choose_tech<R: Rng>(level: u32, special: bool, rng: &mut R) -> Tech {
    if special {
        Tech::Blink
    } else {
        (&[
            Tech::Blink,
            Tech::Blink,
            Tech::Blink,
            Tech::Repel,
            Tech::Repel,
            Tech::Attract,
        ])
            .choose(rng)
            .unwrap()
            .clone()
    }
}

pub fn choose_attack_upgrade<R: Rng>(level: u32, rng: &mut R) -> Attack {
    use Attack::*;
    match level {
        _ => &[Hit(30), Hit(20), Cleave(10), Skewer(10)],
    }
    .choose(rng)
    .unwrap()
    .clone()
}

pub fn choose_defend_upgrade<R: Rng>(level: u32, rng: &mut R) -> Defend {
    use Defend::*;
    match level {
        _ => &[Dodge, Teleport, Revenge],
    }
    .choose(rng)
    .unwrap()
    .clone()
}

pub fn choose_tech_upgrade<R: Rng>(level: u32, rng: &mut R) -> Tech {
    use Tech::*;
    match level {
        _ => &[Blink, CritNext, Attract, Repel, TeleportNext, Skip],
    }
    .choose(rng)
    .unwrap()
    .clone()
}

pub fn choose_curse<R: Rng>(rng: &mut R) -> Outcome {
    use Attack::*;
    use Defend::*;
    use Tech::*;
    (&[
        Outcome::Attack(Miss),
        Outcome::Defend(SkipAttack),
        Outcome::Tech(MissNext),
    ])
        .choose(rng)
        .unwrap()
        .clone()
}
