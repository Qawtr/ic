use candid::{CandidType, Deserialize};

pub type GetSaltResponse = Result<SaltResponse, GetSaltError>;

#[derive(CandidType, Deserialize, Debug, Clone)]
pub enum SaltGenerationStrategy {
    StartOfMonth,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct InitArg {
    pub regenerate_now: bool,
    pub salt_generation_strategy: SaltGenerationStrategy,
    pub registry_polling_interval_secs: u64,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct SaltResponse {
    pub salt: Vec<u8>,
    pub salt_id: u64,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub enum GetSaltError {
    SaltNotInitialized,
    Unauthorized,
    Internal(String),
}
