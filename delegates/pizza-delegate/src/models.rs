use super::*;

/// Origin contract ID - represents the attested identity of the caller
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Origin(pub(crate) Vec<u8>);

impl Origin {
    pub(crate) fn to_b58(&self) -> String {
        bs58::encode(&self.0).into_string()
    }
}

// Storage key constants
pub(crate) const CONTRACT_KEYS_SUFFIX: &str = "::contract_keys";
pub(crate) const SIGNING_KEY_SUFFIX: &str = "::signing_key";
