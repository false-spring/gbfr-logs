use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub enum CharacterType {
    Pl0000,
    Pl0100,
    Pl0200,
    Pl0300,
    Pl0400,
    Pl0500,
    Pl0600,
    Pl0700,
    Pl0800,
    Pl0900,
    Pl1000,
    Pl1100,
    Pl1200,
    Pl1300,
    Pl1400,
    Pl1500,
    Pl1600,
    Pl1700,
    Pl1800,
    Pl1900,
    Pl2000,
    Unknown(u32),
}

impl CharacterType {
    pub fn from_hash(hash: u32) -> Self {
        match hash {
            0x26A4848A => CharacterType::Pl0000,
            0x9498420D => CharacterType::Pl0100,
            0x34D4FD8F => CharacterType::Pl0200,
            0xF8D73D33 => CharacterType::Pl0300,
            0x7B5934AD => CharacterType::Pl0400,
            0x443D46BB => CharacterType::Pl0500,
            0xA9D6569E => CharacterType::Pl0600,
            0xFBA6615D => CharacterType::Pl0700,
            0x63A7C3F0 => CharacterType::Pl0800,
            0xF96A90C2 => CharacterType::Pl0900,
            0x28AC1108 => CharacterType::Pl1000,
            0x94E2514E => CharacterType::Pl1100,
            0x2B4AA114 => CharacterType::Pl1200,
            0xC97F3365 => CharacterType::Pl1300,
            0x601AA977 => CharacterType::Pl1400,
            0xBCC238DE => CharacterType::Pl1500,
            0xC3155079 => CharacterType::Pl1600,
            0xD16CFBDE => CharacterType::Pl1700,
            0x6FDD6932 => CharacterType::Pl1800,
            0x8056ABCD => CharacterType::Pl1900,
            0xF5755C0E => CharacterType::Pl2000,
            _ => CharacterType::Unknown(hash),
        }
    }
}

impl Display for CharacterType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CharacterType::Pl0000 => write!(f, "Pl0000"),
            CharacterType::Pl0100 => write!(f, "Pl0100"),
            CharacterType::Pl0200 => write!(f, "Pl0200"),
            CharacterType::Pl0300 => write!(f, "Pl0300"),
            CharacterType::Pl0400 => write!(f, "Pl0400"),
            CharacterType::Pl0500 => write!(f, "Pl0500"),
            CharacterType::Pl0600 => write!(f, "Pl0600"),
            CharacterType::Pl0700 => write!(f, "Pl0700"),
            CharacterType::Pl0800 => write!(f, "Pl0800"),
            CharacterType::Pl0900 => write!(f, "Pl0900"),
            CharacterType::Pl1000 => write!(f, "Pl1000"),
            CharacterType::Pl1100 => write!(f, "Pl1100"),
            CharacterType::Pl1200 => write!(f, "Pl1200"),
            CharacterType::Pl1300 => write!(f, "Pl1300"),
            CharacterType::Pl1400 => write!(f, "Pl1400"),
            CharacterType::Pl1500 => write!(f, "Pl1500"),
            CharacterType::Pl1600 => write!(f, "Pl1600"),
            CharacterType::Pl1700 => write!(f, "Pl1700"),
            CharacterType::Pl1800 => write!(f, "Pl1800"),
            CharacterType::Pl1900 => write!(f, "Pl1900"),
            CharacterType::Pl2000 => write!(f, "Pl2000"),
            CharacterType::Unknown(_) => write!(f, "Unknown"),
        }
    }
}
