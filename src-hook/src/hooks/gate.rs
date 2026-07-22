//! Damage-gate policy data. The gate that consumes it lives in damage.rs.

/// Sub-entity classes whose damage is simulated on every peer and never arrives
/// as a network-replicated record. The duplicate-frame gate drops a
/// remote-owned source's local-sim calls on the assumption that the replicated
/// record arrives instead; for these classes it never does, so without the
/// exemption their damage is lost on every client except the owner's.
///
/// A wrong entry disables duplicate-frame suppression for a class that needs
/// it, so add one only on a paired owner/observer capture showing the owner
/// logging hits the observers lose entirely. Reflection ancestry is not the
/// test: Ferry's main pet (`0x2AF678E8`) shares it and is replicated.
const LOCAL_SIM_ONLY_CLASSES: &[u32] = &[
    0x5B1AB457, // Wp2290               - Seofon's avatar sword
    0x8364C8BC, // Pl0700GhostSatellite - Ferry's Umlauf
    0xC9F45042, // Wp1890               - Cagliostro's Ouroboros (PT/Alexandria)
];

pub(crate) fn is_local_sim_only_class(source_type_id: u32) -> bool {
    LOCAL_SIM_ONLY_CLASSES.contains(&source_type_id)
}
