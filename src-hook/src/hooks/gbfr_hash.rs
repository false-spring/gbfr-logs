//! The game's custom XXHash32: hashing a class-name string yields that class's
//! u32 type-id. The constants below replace stock XXHash32's seed derivation.
//!
//! Used to disambiguate enemy variants, summons, character types (PL####), and
//! enemy sub-names e.g. Em8200_01

const P1: u32 = 0x9E37_79B1;
const P2: u32 = 0x85EB_CA77;
const P3: u32 = 0xC2B2_AE3D;
const P4: u32 = 0x27D4_EB2F;
const P5: u32 = 0x1656_67B1;

const SEED: u32 = 0x178A_54A4;
const V1_0: u32 = 0x2557_311B;
const V2_0: u32 = 0x871F_B76A;
const V3_0: u32 = 0x0133_ECF3;
const V4_0: u32 = 0x62FC_7342;

#[inline(always)]
fn round(acc: u32, lane: u32) -> u32 {
    let acc = acc.wrapping_add(lane.wrapping_mul(P2));
    let acc = acc.rotate_left(13);
    acc.wrapping_mul(P1)
}

#[inline(always)]
fn read_u32_le(data: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]])
}

pub fn gbfr_hash_bytes(data: &[u8]) -> u32 {
    let l = data.len();
    let mut i = 0usize;

    let mut h = if l >= 16 {
        let (mut v1, mut v2, mut v3, mut v4) = (V1_0, V2_0, V3_0, V4_0);
        while i + 16 <= l {
            v1 = round(v1, read_u32_le(data, i));
            v2 = round(v2, read_u32_le(data, i + 4));
            v3 = round(v3, read_u32_le(data, i + 8));
            v4 = round(v4, read_u32_le(data, i + 12));
            i += 16;
        }
        v1.rotate_left(1)
            .wrapping_add(v2.rotate_left(7))
            .wrapping_add(v3.rotate_left(12))
            .wrapping_add(v4.rotate_left(18))
    } else {
        // Stock uses seed + PRIME32_5 below 16 bytes, GBFR the bare seed.
        SEED
    };

    h = h.wrapping_add(l as u32);

    while i + 4 <= l {
        h = h.wrapping_add(read_u32_le(data, i).wrapping_mul(P3));
        h = h.rotate_left(17).wrapping_mul(P4);
        i += 4;
    }
    while i < l {
        h = h.wrapping_add((data[i] as u32).wrapping_mul(P5));
        h = h.rotate_left(11).wrapping_mul(P1);
        i += 1;
    }

    h ^= h >> 15;
    h = h.wrapping_mul(P2);
    h ^= h >> 13;
    h = h.wrapping_mul(P3);
    h ^= h >> 16;
    h
}

#[inline]
pub fn gbfr_hash(s: &str) -> u32 {
    gbfr_hash_bytes(s.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reproduces_known_type_id_pairs() {
        assert_eq!(gbfr_hash(""), 0x887A_E0B0);

        assert_eq!(gbfr_hash("We7700"), 0xD495_0D74); // Refutation
        assert_eq!(gbfr_hash("We7701"), 0x2BD7_220B); // Repudiation
        assert_eq!(gbfr_hash("We7702"), 0x635F_FFB6); // Renunciation

        assert_eq!(gbfr_hash("Em7700"), 0x2B31_654B);

        assert_eq!(gbfr_hash("Pl0700Ghost"), 0x2AF6_78E8);
        assert_eq!(gbfr_hash("Wp2290"), 0x5B1A_B457);
        assert_eq!(gbfr_hash("Pl2000"), 0xF575_5C0E);
        assert_eq!(gbfr_hash("So0200"), 0x6D06_8BDE);
    }
}
