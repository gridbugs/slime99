use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Attack {
    Hit(u32),
    Cleave(u32),
    Skewer(u32),
    Miss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Defend {
    Dodge,
    Teleport,
    Revenge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tech {
    Blink,
    CritNext,
    Attract,
    Repel,
    MissNext,
    TeleportNext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbilityTarget {
    Attack,
    Defend,
    Tech,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ability {
    Stash(AbilityTarget),
    SwapTop2(AbilityTarget),
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

impl Player {
    pub fn new() -> Self {
        Self {
            attack: Deck {
                items: rev(vec![
                    Attack::Hit(4),
                    Attack::Hit(4),
                    Attack::Hit(4),
                    Attack::Hit(4),
                    Attack::Skewer(12),
                    Attack::Miss,
                    Attack::Hit(4),
                    Attack::Cleave(4),
                    Attack::Hit(99),
                    Attack::Skewer(4),
                ]),
                max_size: 16,
            },
            defend: Deck {
                items: rev(vec![
                    Defend::Dodge,
                    Defend::Dodge,
                    Defend::Dodge,
                    Defend::Teleport,
                    Defend::Revenge,
                    Defend::Dodge,
                    Defend::Revenge,
                    Defend::Dodge,
                    Defend::Dodge,
                    Defend::Teleport,
                    Defend::Dodge,
                ]),
                max_size: 16,
            },
            tech: Deck {
                items: rev(vec![
                    Tech::Blink,
                    Tech::Attract,
                    Tech::Blink,
                    Tech::Repel,
                    Tech::MissNext,
                    Tech::Blink,
                    Tech::CritNext,
                    Tech::TeleportNext,
                ]),
                max_size: 8,
            },
            ability: AbilityTable {
                abilities: vec![
                    Ability::SwapTop2(AbilityTarget::Attack),
                    Ability::Stash(AbilityTarget::Defend),
                    Ability::Stash(AbilityTarget::Attack),
                    Ability::SwapTop2(AbilityTarget::Defend),
                    Ability::Stash(AbilityTarget::Tech),
                ],
                max_size: 8,
            },
        }
    }
}
