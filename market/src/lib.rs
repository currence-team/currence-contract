use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::Vector;
use near_sdk::json_types::U128;
use near_sdk::{env, near_bindgen, AccountId, Balance, Promise, PromiseOrValue};

use crate::market::*;

mod constants;
mod errors;
mod instructions;
mod lmsr;
mod market;
mod storage_impl;
mod token_receiver;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Contract {
    markets: Vector<Market>,
}

impl Default for Contract {
    fn default() -> Self {
        Self {
            markets: Vector::new(b"near-prediction".to_vec()),
        }
    }
}

type MarketId = u64;

#[near_bindgen]
impl Contract {
    #[payable]
    pub fn create_market(&mut self, args: CreateMarketArgs) -> MarketId {
        let market_id: MarketId = self.markets.len();
        let market = Market::new(market_id, args);
        self.markets.push(&market);
        market_id
    }

    pub fn get_markets(&self) -> u64 {
        self.markets.len()
    }

    fn get_market(&self, market_id: u64) -> Market {
        self.markets.get(market_id).unwrap()
    }

    pub fn open_market(&mut self, market_id: MarketId) {
        let mut market = self.get_market(market_id);
        assert_eq!(market.operator, env::signer_account_id());
        market.open();
        self.markets.replace(market.id, &market);
    }

    pub fn pause_market(&mut self, market_id: MarketId) {
        let mut market = self.get_market(market_id);
        market.pause();
    }

    pub fn resolve_market(&mut self, market_id: MarketId, payouts: Vec<u128>) {
        let mut market = self.get_market(market_id);
        assert!(market.stage == Stage::Paused || market.stage == Stage::Open);
        assert_eq!(market.outcomes.len(), payouts.len() as u64);

        // TODO(cqsd): wait for merge
        // let expected_payout_vec_sum: u128 = 10u128.pow(market.collateral_decimals);
        // use this as the match guard
        match payouts.iter().sum::<u128>() {
            s if (s == market.collateral_decimals as u128) => {
                // usual case, resolve the market
                market.payouts = Some(payouts);
                market.stage = Stage::Finalized(Finalization::Resolved);
            }
            0 => {
                // no payouts --> the outcomes were invalid, put market in refund mode
                market.payouts = None;
                market.stage = Stage::Finalized(Finalization::Invalid);
                // TODO(cqsd): need to handle the refund state in redeem?
            }
            _ => env::panic(b"Invalid payout vector"),
        };

        self.markets.replace(market_id, &market);
    }

    pub fn buy(
        &mut self,
        sender_id: &AccountId,
        token_id: &AccountId,
        amount: Balance,
        ix: instructions::Buy,
    ) -> PromiseOrValue<U128> {
        let mut market = self.get_market(ix.market_id.into());
        assert_eq!(market.collateral_token, *token_id);

        let ret = market.internal_buy(&sender_id, amount, ix.num_shares, ix.outcome_id);
        self.markets.replace(market.id, &market);

        ret
    }

    pub fn sell(
        &mut self,
        sender_id: &AccountId,
        token_id: &AccountId,
        amount: Balance,
        ix: instructions::Sell,
    ) {
        let mut market = self.get_market(ix.market_id.into());
        assert_eq!(market.collateral_token, *token_id);

        market.internal_sell(&sender_id, amount, ix.num_shares, ix.outcome_id);
        self.markets.replace(market.id, &market);
    }

    pub fn deposit(
        &mut self,
        _sender_id: &AccountId,
        token_id: &AccountId,
        amount: Balance,
        ix: instructions::InitialDeposit,
    ) -> PromiseOrValue<U128> {
        let mut market = self.get_market(ix.market_id.into());
        assert_eq!(market.collateral_token, *token_id);
        market.deposit_collateral(amount);

        self.markets.replace(market.id, &market);

        PromiseOrValue::Value(U128(0))
    }

    pub fn withdraw_fees(&mut self, market_id: MarketId) -> Promise {
        let mut market = self.get_market(market_id.into());
        let ret = market.withdraw_fees();

        self.markets.replace(market.id, &market);

        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::MockedBlockchain;
    use near_sdk::{testing_env, VMContext};

    const CURRENT_ACCOUNT_ID: &'static str = "contract.testnet";
    const SIGNER_ACCOUNT_ID: &'static str = "alice.testnet";
    const PREDECESSOR_ACCOUNT_ID: &'static str = "alice.testnet";
    const ONE_HOUR_NS: u64 = 60 * 60 * 1_000_000_000;

    fn get_context(input: Vec<u8>, is_view: bool) -> VMContext {
        VMContext {
            current_account_id: CURRENT_ACCOUNT_ID.to_string(),
            signer_account_id: SIGNER_ACCOUNT_ID.to_string(),
            signer_account_pk: vec![0, 1, 2],
            predecessor_account_id: PREDECESSOR_ACCOUNT_ID.to_string(),
            input,
            block_index: 0,
            block_timestamp: 0,
            account_balance: 0,
            account_locked_balance: 0,
            storage_usage: 0,
            attached_deposit: 0,
            prepaid_gas: 10u64.pow(18),
            random_seed: vec![0, 1, 2],
            is_view,
            output_data_receivers: vec![],
            epoch_height: 19,
        }
    }

    fn create_test_market(num_outcomes: u32) -> CreateMarketArgs {
        CreateMarketArgs {
            title: "Will Donald Trump win the 2024 US Election?".into(),
            description:
                "This question will be settled based on Associated Press (AP) election calls."
                    .into(),
            collateral_token: "test.near".into(),
            collateral_decimals: 9,
            trade_fee_bps: 1,
            resolution_time: env::block_timestamp() + ONE_HOUR_NS,
            end_time: env::block_timestamp() + ONE_HOUR_NS,
            fee_owner: None,
            oracle: None,
            operator: None,
            outcomes: (0..num_outcomes)
                .map(|i| Outcome {
                    id: i,
                    short_name: "Test".into(),
                    long_name: "Test".into(),
                })
                .collect(),
        }
    }

    #[test]
    fn add_market() {
        let context = get_context(vec![], false);
        testing_env!(context);
        let mut contract = Contract {
            markets: Vector::new(b"mk".to_vec()),
        };
        assert_eq!(0, contract.get_markets());
        let args = create_test_market(2);
        contract.create_market(args);
        assert_eq!(1, contract.get_markets());
    }

    #[test]
    fn buy_shares() {
        let context = get_context(vec![], false);
        testing_env!(context);
        let mut contract = Contract {
            markets: Vector::new(b"mk".to_vec()),
        };
        let args = create_test_market(2);
        let market_id = contract.create_market(args);
        let account_id: AccountId = SIGNER_ACCOUNT_ID.into();
        let mut market = contract.markets.get(market_id).unwrap();
        market.deposit_collateral(100_000_000_000);
        market.open();
        assert_eq!(None, market.outcome_balance(&account_id, 0));
        assert_eq!(None, market.outcome_balance(&account_id, 1));
        market.credit(&account_id, 0, 5);
        assert_eq!(Some(5), market.outcome_balance(&account_id, 0));
        assert_eq!(Some(0), market.outcome_balance(&account_id, 1));
    }

    #[test]
    fn buy_price_increase() {
        let context = get_context(vec![], false);
        testing_env!(context);
        let mut contract = Contract {
            markets: Vector::new(b"mk".to_vec()),
        };
        let args = create_test_market(2);
        let market_id = contract.create_market(args);
        let market = contract.markets.get(market_id).unwrap();
        let buy_price = market.calc_price_without_fee(0, 10, OrderDirection::Buy);
        assert!(buy_price > 5_200_000_000);
    }

    #[test]
    fn sell_price_decrease() {
        let context = get_context(vec![], false);
        testing_env!(context);
        let mut contract = Contract {
            markets: Vector::new(b"mk".to_vec()),
        };
        let args = create_test_market(2);
        let market_id = contract.create_market(args);
        let mut market = contract.markets.get(market_id).unwrap();
        let account_id: AccountId = "test_account".into();
        market.deposit_collateral(100_000_000_000);
        market.open();
        market.credit(&account_id, 1, 100);
        // Selling more shares will reduce the average price
        let max_sell_price = market.calc_sell_price(1, 10) / 10;
        let mid_sell_price = market.calc_sell_price(1, 50) / 50;
        let min_sell_price = market.calc_sell_price(1, 100) / 100;
        assert!(min_sell_price < mid_sell_price);
        assert!(mid_sell_price < max_sell_price);
    }
}
