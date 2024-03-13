pub mod constants;
pub mod v0;

#[allow(dead_code)]
pub mod v1;

pub fn deserialize_version(data: &[u8], version: u8) -> anyhow::Result<v1::Parser> {
    match version {
        0 => Ok(v0::Parser::from_blob(data)?.into()),
        1 => Ok(v1::Parser::from_encounter_blob(data)?),
        _ => Err(anyhow::anyhow!("Unknown version")),
    }
}
