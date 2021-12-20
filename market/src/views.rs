use crate::*;
use near_sdk::near_bindgen;
use near_sdk::serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct MarketView {
    pub id: u64,
    pub title: String,
    pub description: String,

    pub collateral_token: AccountId,
    pub collateral_decimals: u32,
    pub deposited_collateral: Balance,
    pub minimum_deposit: Balance,

    pub end_time: Timestamp,
    pub resolution_time: Timestamp,

    pub outcomes: Vec<OutcomeView>,
    /// Number of outstanding shares per outcome
    pub shares: Vec<f64>,
    pub stage: Stage,
    pub trade_fee_bps: u16,
    /// Running tally of total trade volume
    pub volume: Balance,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct OutcomeView {
    pub id: OutcomeId,
    pub short_name: String,
    pub long_name: String,
    pub price: u128,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct BalanceView {
    pub market_id: u64,
    pub outcome_id: OutcomeId,
    pub shares: u128,
}

impl Market {
    pub fn into_view(self) -> MarketView {
        let prices = self.calculate_prices();
        return MarketView {
            id: self.id,
            title: self.title,
            description: self.description,
            collateral_token: self.collateral_token,
            collateral_decimals: self.collateral_decimals,
            deposited_collateral: self.deposited_collateral,
            minimum_deposit: self.minimum_deposit,
            end_time: self.end_time,
            resolution_time: self.resolution_time,
            outcomes: self
                .outcomes
                .iter()
                .zip(prices)
                .map(|(o, p)| OutcomeView {
                    id: o.id,
                    short_name: o.short_name,
                    long_name: o.long_name,
                    price: p,
                })
                .collect(),
            shares: self.shares,
            stage: self.stage,
            trade_fee_bps: self.trade_fee_bps,
            volume: self.volume,
        };
    }

    pub fn get_user_balances(&self, account_id: AccountId) -> Vec<BalanceView> {
        let balance = self.accounts.get(&account_id);
        match balance {
            None => vec![],
            Some(balances) => balances
                .iter()
                .enumerate()
                .map(|(idx, balance)| BalanceView {
                    market_id: self.id,
                    outcome_id: idx as u32,
                    shares: *balance,
                })
                .collect(),
        }
    }
}

#[near_bindgen]
impl Contract {
    pub fn get_market_info(self, market_id: u64) -> MarketView {
        let market = self.markets.get(market_id).unwrap();
        return market.into_view();
    }

    pub fn get_all_markets(self) -> Vec<MarketView> {
        return self.markets.iter().map(|m| m.into_view()).collect();
    }

    pub fn get_user_balances(self, account_id: AccountId) -> Vec<BalanceView> {
        return self
            .markets
            .iter()
            .map(|m| m.get_user_balances(account_id.clone()))
            .flatten()
            .collect();
    }
}
