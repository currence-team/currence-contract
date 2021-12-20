use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::Balance;

/// Message parameters to receive via token function call.
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[serde(tag = "type")]
pub enum Instruction {
    Buy(Buy),
    InitialDeposit(InitialDeposit),
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Buy {
    pub market_id: u64,
    pub outcome_id: u32,
    pub num_shares: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Sell {
    pub market_id: u64,
    pub outcome_id: u32,
    pub num_shares: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct InitialDeposit {
    pub market_id: u64,
}
