use serde::{Deserialize, Serialize};
use strum_macros::Display;

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone, Copy, Display)]
pub enum CharacterType {
    /// Gran
    Pl0000,
    /// Djeeta
    Pl0100,
    /// Katalina
    Pl0200,
    /// Rackam
    Pl0300,
    /// Io
    Pl0400,
    /// Eugen
    Pl0500,
    /// Rosetta
    Pl0600,
    /// Ferry
    Pl0700,
    /// Lancelot
    Pl0800,
    /// Vane
    Pl0900,
    /// Percival
    Pl1000,
    /// Siegfried
    Pl1100,
    /// Charlotta
    Pl1200,
    /// Yodarha
    Pl1300,
    /// Narmaya
    Pl1400,
    /// Ghandagoza
    Pl1500,
    /// Zeta
    Pl1600,
    /// Vaseraga
    Pl1700,
    /// Cagliostro
    Pl1800,
    /// Id
    Pl1900,
    /// Id (Transformation)
    Pl2000,
    /// Sandalphon
    Pl2100,
    /// Seofon
    Pl2200,
    /// Tweyen
    Pl2300,
    /// Ferry Ghost
    Pl0700Ghost,
    /// Ferry Ghost (Satellite) / Umlauf
    Pl0700GhostSatellite,
    #[strum(default)]
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
            0x9C89A455 => CharacterType::Pl2100,
            0x59DB0CD9 => CharacterType::Pl2200,
            0xDA5A8E25 => CharacterType::Pl2300,
            0x2AF678E8 => CharacterType::Pl0700Ghost,
            0x8364C8BC => CharacterType::Pl0700GhostSatellite,
            _ => CharacterType::Unknown(hash),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone, Copy, Display)]
pub enum EnemyType {
    #[strum(default)]
    Unknown(u32),
}

impl EnemyType {
    pub fn from_hash(hash: u32) -> Self {
        EnemyType::Unknown(hash)
    }
}

#[repr(u32)]
#[derive(Copy, Clone)]
pub enum FerrySkillId {
    PetNormal = 65u32,
    BlausGespenst = 1100u32,
    Pendel = 1400u32,
    Strafe = 1500u32,
}

