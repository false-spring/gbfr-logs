use std::collections::HashMap;
use std::sync::OnceLock;

use crate::parser::v1::MasterTraitFlag;

/// Style bit assignments for the `pN_styles` DB columns and the style filter.
pub const STYLE_INSIGHT: u8 = 1 << 0; // SB_DEF
pub const STYLE_ESSENCE: u8 = 1 << 1; // SB_ATK
pub const STYLE_CRUX: u8 = 1 << 2; // SB_LIMIT

const BOARD_COUNT: usize = 3;
const RANK_GROUP_COUNT: usize = 3;

fn board_index(board: &str) -> Option<usize> {
    match board {
        "SB_DEF" => Some(0),
        "SB_ATK" => Some(1),
        "SB_LIMIT" => Some(2),
        _ => None,
    }
}

pub fn style_filter_mask(style: &str) -> Option<u8> {
    match style {
        "SB_DEF" => Some(STYLE_INSIGHT),
        "SB_ATK" => Some(STYLE_ESSENCE),
        "SB_LIMIT" => Some(STYLE_CRUX),
        _ => None,
    }
}

/// SBE hash -> (board index, rank-group index) for the cost-100 rows, the
/// board's "Style Rank Perk Bonuses".
fn perk_index() -> &'static HashMap<u32, (usize, usize)> {
    static INDEX: OnceLock<HashMap<u32, (usize, usize)>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let raw: HashMap<String, serde_json::Value> =
            serde_json::from_str(include_str!("../assets/master-traits.json"))
                .expect("bundled master-traits.json is valid JSON");

        let mut index = HashMap::new();
        for (key, node) in raw {
            // Non-hex keys (the `_meta` block) are not catalog entries.
            let Ok(hash) = u32::from_str_radix(&key, 16) else {
                continue;
            };
            if node.get("cost").and_then(|v| v.as_u64()) != Some(100) {
                continue;
            }
            let Some(board) = node.get("board").and_then(|v| v.as_str()).and_then(board_index)
            else {
                continue;
            };
            let group = match node.get("group").and_then(|v| v.as_str()) {
                Some("rank1") => 0,
                Some("rank2") => 1,
                Some("rank3") => 2,
                _ => continue,
            };
            index.insert(hash, (board, group));
        }
        index
    })
}

pub fn completed_styles(flags: &[MasterTraitFlag]) -> u8 {
    let index = perk_index();
    let mut group_on = [[false; RANK_GROUP_COUNT]; BOARD_COUNT];

    for flag in flags {
        if !flag.on {
            continue;
        }
        if let Some(&(board, group)) = index.get(&flag.hash) {
            group_on[board][group] = true;
        }
    }

    let mut mask = 0;
    for (board, groups) in group_on.iter().enumerate() {
        if groups.iter().all(|&on| on) {
            mask |= 1 << board;
        }
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flag(hash: u32, on: bool) -> MasterTraitFlag {
        MasterTraitFlag { hash, on }
    }

    // Perk-row hashes straight out of master-traits.json.
    const DEF_PERKS: [u32; 3] = [0x3a583a9c, 0xfce3d57d, 0x1ad4cc16];
    const ATK_PERKS: [u32; 3] = [0xec41afa1, 0x8130d9ef, 0x120d7a61];
    const DEF_GEM_NODE: u32 = 0xff561659; // cost-50, must not count

    #[test]
    fn completed_styles_requires_all_three_perk_rows() {
        let all_on: Vec<_> = DEF_PERKS.iter().map(|&h| flag(h, true)).collect();
        assert_eq!(completed_styles(&all_on), STYLE_INSIGHT);

        let one_off = vec![
            flag(DEF_PERKS[0], true),
            flag(DEF_PERKS[1], true),
            flag(DEF_PERKS[2], false),
        ];
        assert_eq!(completed_styles(&one_off), 0);

        let one_missing = vec![flag(DEF_PERKS[0], true), flag(DEF_PERKS[1], true)];
        assert_eq!(completed_styles(&one_missing), 0);
    }

    #[test]
    fn completed_styles_ignores_gem_nodes_and_unknown_hashes() {
        let flags = vec![
            flag(DEF_PERKS[0], true),
            flag(DEF_PERKS[1], true),
            flag(DEF_GEM_NODE, true),
            flag(0xdeadbeef, true),
        ];
        assert_eq!(completed_styles(&flags), 0);
    }

    #[test]
    fn completed_styles_tracks_boards_independently() {
        let mut flags: Vec<_> = DEF_PERKS.iter().map(|&h| flag(h, true)).collect();
        flags.extend(ATK_PERKS.iter().map(|&h| flag(h, true)));
        assert_eq!(completed_styles(&flags), STYLE_INSIGHT | STYLE_ESSENCE);

        assert_eq!(completed_styles(&[]), 0);
    }

    #[test]
    fn style_filter_mask_maps_boards() {
        assert_eq!(style_filter_mask("SB_DEF"), Some(STYLE_INSIGHT));
        assert_eq!(style_filter_mask("SB_ATK"), Some(STYLE_ESSENCE));
        assert_eq!(style_filter_mask("SB_LIMIT"), Some(STYLE_CRUX));
        assert_eq!(style_filter_mask("bogus"), None);
    }
}
