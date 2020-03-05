use game::CardinalDirection;
use maplit::hashmap;
use prototty::input::KeyboardInput;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize)]
pub enum AppInput {
    Move(CardinalDirection),
    Tech,
    Wait,
    Ability(u8),
    Examine,
}

#[derive(Serialize, Deserialize)]
pub struct Controls {
    keys: HashMap<KeyboardInput, AppInput>,
}

impl Controls {
    pub fn default() -> Self {
        let keys = hashmap![
            KeyboardInput::Left => AppInput::Move(CardinalDirection::West),
            KeyboardInput::Right => AppInput::Move(CardinalDirection::East),
            KeyboardInput::Up => AppInput::Move(CardinalDirection::North),
            KeyboardInput::Down => AppInput::Move(CardinalDirection::South),
            KeyboardInput::Char('t') => AppInput::Tech,
            KeyboardInput::Char('x') => AppInput::Examine,
            KeyboardInput::Char(' ') => AppInput::Wait,
            KeyboardInput::Char('1') => AppInput::Ability(0),
            KeyboardInput::Char('2') => AppInput::Ability(1),
            KeyboardInput::Char('3') => AppInput::Ability(2),
            KeyboardInput::Char('4') => AppInput::Ability(3),
            KeyboardInput::Char('5') => AppInput::Ability(4),
            KeyboardInput::Char('6') => AppInput::Ability(5),
            KeyboardInput::Char('7') => AppInput::Ability(6),
            KeyboardInput::Char('8') => AppInput::Ability(7),
        ];
        Self { keys }
    }

    pub fn get(&self, keyboard_input: KeyboardInput) -> Option<AppInput> {
        self.keys.get(&keyboard_input).cloned()
    }
}
