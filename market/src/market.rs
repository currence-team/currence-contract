use std::convert::TryInto;

use near_contract_standards::fungible_token::core_impl::ext_fungible_token;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, Vector};
use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, AccountId, Balance, Promise, PromiseOrValue};

use crate::constants::*;
use crate::lmsr;

pub type Timestamp = u64;

#[derive(Debug, PartialEq, BorshDeserialize, BorshSerialize)]
pub enum Stage {
    /// The market has never been opened.
    Pending,
    /// The market is open for trading.
    Open,
    /// Trading is paused.
    Paused,
    /// The market has been resolved.
    Finalized(Finalization),
}

#[derive(Debug, PartialEq, BorshDeserialize, BorshSerialize)]
pub enum Finalization {
    Resolved,
    Invalid,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Outcome {
    pub id: OutcomeId,
    pub short_name: String,
    pub long_name: String,
}

pub type OutcomeId = u32;

#[derive(BorshDeserialize, BorshSerialize)]
pub struct Market {
    pub id: u64,
    pub title: String,
    pub description: String,

    pub collateral_token: AccountId,
    pub collateral_decimals: u32,
    pub deposited_collateral: Balance,
    pub minimum_deposit: Balance,

    /// unix ts in nanoseconds
    pub end_time: Timestamp,
    /// unix ts in nanoseconds
    pub resolution_time: Timestamp,

    pub outcomes: Vector<Outcome>,
    /// Number of outstanding shares per outcome
    pub shares: Vec<f64>,
    /// Payout weights. For a valid market, weights must sum to 1 of the
    /// collateral token taking in terms of its precision (e.g., if collateral
    /// has 18 decimals, must sum to 10^18). For invalid markets, weights must
    /// all be 0.
    pub payouts: Option<Vec<Balance>>,

    /// Account responsible for resolving the market
    pub oracle: AccountId,
    /// Account responsible for making admin changes, such as starting the
    /// market or editing the description
    pub operator: AccountId,

    pub stage: Stage,

    pub fee_owner: AccountId,
    pub trade_fee_bps: u16,
    /// Pool fees accrued
    pub fees_accrued: Balance,
    /// Running tally of total trade volume
    pub volume: Balance,

    /// Outcome token balances of market participants
    pub accounts: LookupMap<AccountId, OutcomeBalance>,
}

/// A type representing outcome token balances of a market participant. The
/// outcome ID is used to index balances
pub type OutcomeBalance = Vec<Balance>;

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct CreateMarketArgs {
    pub title: String,
    pub description: String,

    pub collateral_token: AccountId,
    pub collateral_decimals: u32,
    pub end_time: Timestamp,
    pub resolution_time: Timestamp,
    pub trade_fee_bps: u16,

    pub outcomes: Vec<Outcome>,

    pub fee_owner: Option<AccountId>,
    pub operator: Option<AccountId>,
    pub oracle: Option<AccountId>,
}

pub enum OrderDirection {
    Buy,
    Sell,
}

impl Market {
    pub fn new(id: u64, args: CreateMarketArgs) -> Self {
        let mut outcomes = Vector::new(b"outcomes".to_vec());
        outcomes.extend(args.outcomes);

        let creator = env::signer_account_id();
        let fee_owner = args.fee_owner.unwrap_or(creator.clone());
        let operator = args.operator.unwrap_or(creator.clone());
        let oracle = args.oracle.unwrap_or(operator.clone());
        let shares = vec![0.; outcomes.len().try_into().unwrap()];

        Self {
            id,
            title: args.title,
            description: args.description,

            outcomes,
            payouts: None,

            end_time: args.end_time,
            resolution_time: args.resolution_time,
            stage: Stage::Pending,

            // TODO(sbb): append something market specific to key
            accounts: LookupMap::new(b"acc_map".to_vec()),
            collateral_token: args.collateral_token,
            collateral_decimals: args.collateral_decimals,
            deposited_collateral: 0,
            minimum_deposit: MINIMUM_DEPOSIT * (10 as u128).pow(args.collateral_decimals),

            trade_fee_bps: args.trade_fee_bps,
            fees_accrued: 0,
            volume: 0,

            fee_owner,
            operator,
            oracle,
            shares,
        }
    }

    pub fn open(&mut self) {
        self.validate();
        self.assert_stages(&[Stage::Paused, Stage::Pending]);
        self.stage = Stage::Open;
    }

    pub fn pause(&mut self) {
        self.assert_stage(Stage::Open);
        self.stage = Stage::Paused;
    }

    pub fn deposit_collateral(&mut self, amount: u128) {
        // TODO(sbb): Support paused as well?
        self.assert_stage(Stage::Pending);
        self.deposited_collateral += amount;
    }

    pub fn calc_buy_price(&self, outcome_id: OutcomeId, num_shares: Balance) -> Balance {
        let base_price = self.calc_price_without_fee(outcome_id, num_shares, OrderDirection::Buy);
        let fee = self.calc_fee(base_price);
        base_price.checked_add(fee).unwrap()
    }

    pub fn calc_sell_price(&self, outcome_id: OutcomeId, num_shares: Balance) -> Balance {
        let base_price = self.calc_price_without_fee(outcome_id, num_shares, OrderDirection::Sell);
        let fee = self.calc_fee(base_price);
        base_price.checked_add(fee).unwrap()
    }

    pub fn calc_price_without_fee(
        &self,
        outcome_id: OutcomeId,
        num_shares: Balance,
        direction: OrderDirection,
    ) -> Balance {
        let multiplier = match direction {
            OrderDirection::Buy => 1.0,
            OrderDirection::Sell => -1.0,
        };
        // e.g. 5.2493 (average 0.52 per share for uninitialized market)
        let estimate = lmsr::estimate(
            50.0,
            &self.shares,
            outcome_id.try_into().unwrap(),
            multiplier * (num_shares as f64),
        )
        .abs();

        // 5.24 * 10 -> 52.4 -> 53 for buy, 52 for sell
        let rounded = match direction {
            OrderDirection::Buy => (estimate * 10.0).ceil() as u128,
            OrderDirection::Sell => (estimate * 10.0).floor() as u128,
        };
        let base: u128 = 10;
        // Multiplied by 10 above so exponent should be 1 less
        let total = rounded * base.checked_pow(self.collateral_decimals - 1).unwrap();
        return total;
    }

    pub fn calc_fee(&self, base_price: Balance) -> Balance {
        base_price
            .checked_mul(self.trade_fee_bps.into())
            .unwrap()
            .checked_div(self.collateral_decimals.into())
            .unwrap()
    }

    pub fn deposit_fees(&mut self, amount: Balance) {
        self.fees_accrued = self.fees_accrued.checked_add(amount).unwrap();
    }

    pub fn withdraw_fees(&mut self) -> Promise {
        assert!(self.fees_accrued > 0);

        let fees = self.fees_accrued;
        self.fees_accrued = 0;

        ext_fungible_token::ft_transfer(
            self.fee_owner.clone(),
            U128(fees),
            Some(format!("Withdrawing {} fees to {}", fees, self.fee_owner)),
            &self.collateral_token,
            ONE_YOCTO,
            GAS_FOR_FT_TRANSFER,
        )
    }

    pub fn get_or_create_balances(&mut self, account_id: &AccountId) -> OutcomeBalance {
        match self.accounts.get(account_id) {
            Some(a) => a,
            None => vec![0; self.outcomes.len() as usize],
        }
    }

    /// Burn all outcome tokens and redeem for collateral
    pub fn redeem(&mut self, account_id: &AccountId) -> Promise {
        self.assert_finalized();
        // TODO(cqsd): error
        let balances = self.accounts.get(&account_id).expect("no such account");
        let payout = match &self.payouts {
            Some(p) => balances
                .iter()
                .zip(p.iter())
                .map(|(b, &p)| b.checked_mul(p).unwrap())
                .sum(),
            None => 0,
        };
        assert!(payout > 0);

        // TODO(cqsd): handle invalid case (payout == 0)
        // in that case, outcome tokens are redeemed for equal shares of the pool.
        // we need to keep track of the total number of shares in circulation for this, which could be a problem...

        ext_fungible_token::ft_transfer(
            account_id.into(),
            U128(payout),
            Some(format!(
                "Redeeming {} winning tokens for {}",
                payout,
                env::current_account_id()
            )),
            &self.collateral_token,
            ONE_YOCTO,
            GAS_FOR_FT_TRANSFER,
        )
    }

    pub fn credit(&mut self, account_id: &AccountId, outcome_id: OutcomeId, num_shares: Balance) {
        self.assert_trading_allowed();

        let mut balances = self.get_or_create_balances(&account_id);
        balances[outcome_id as usize] += num_shares;
        self.accounts.insert(&account_id, &balances);
    }

    pub fn debit(&mut self, account_id: &AccountId, outcome_id: OutcomeId, num_shares: Balance) {
        self.assert_trading_allowed();

        let mut balances = self.get_or_create_balances(&account_id);
        let new_balance = match balances[outcome_id as usize] {
            s if (s < num_shares) => panic!("shit"),
            old => old - num_shares,
        };
        balances[outcome_id as usize] = new_balance;

        self.accounts.insert(&account_id, &balances);
    }

    pub fn outcome_balance(
        &self,
        account_id: &AccountId,
        outcome_id: OutcomeId,
    ) -> Option<Balance> {
        self.accounts
            .get(account_id)?
            .get(outcome_id as usize)
            .copied()
    }
}

type ReceiverResponse = PromiseOrValue<U128>;

// internal methods
impl Market {
    pub fn internal_buy(
        &mut self,
        sender_id: &AccountId,
        amount: Balance,
        num_shares: Balance,
        outcome_id: OutcomeId,
    ) -> ReceiverResponse {
        self.assert_trading_allowed();
        assert!(self.outcomes.len() > outcome_id.into());

        let base_price = self.calc_price_without_fee(outcome_id, num_shares, OrderDirection::Buy);
        let fee = self.calc_fee(base_price);
        let cost = base_price.checked_add(fee).unwrap();
        if amount < cost {
            // not enough collateral for this buy, cancel the whole thing
            return PromiseOrValue::Value(U128(amount));
        }
        // credit the user outcome share balance and return excess collateral
        self.credit(sender_id, outcome_id, num_shares);
        self.deposit_fees(fee);
        return PromiseOrValue::Value(U128(amount - cost));
    }

    pub fn internal_sell(
        &mut self,
        sender_id: &AccountId,
        amount: Balance,
        num_shares: Balance,
        outcome_id: OutcomeId,
    ) {
        self.assert_trading_allowed();
        assert!(self.outcomes.len() > outcome_id.into());

        let base_price = self.calc_price_without_fee(outcome_id, num_shares, OrderDirection::Sell);
        let fee = self.calc_fee(base_price);
        let sell_amount = base_price.checked_sub(fee).unwrap();
        if sell_amount < amount {
            panic!("Not executing transaction due to slippage");
        }
        // credit the user outcome share balance and return excess collateral
        self.debit(sender_id, outcome_id, num_shares);
        self.deposit_fees(fee);

        ext_fungible_token::ft_transfer(
            sender_id.clone(),
            U128(sell_amount),
            Some(format!(
                "Paying {} for {} shares to {}",
                sell_amount, num_shares, sender_id
            )),
            &self.collateral_token,
            ONE_YOCTO,
            GAS_FOR_FT_TRANSFER,
        );
    }
}

// validation
impl Market {
    fn validate(&self) {
        assert!(self.outcomes.len() > 0);
        assert!(self.end_time > env::block_timestamp());
        assert!(self.resolution_time > env::block_timestamp());
        println!(
            "deposited: {} min: {}",
            self.deposited_collateral, self.minimum_deposit
        );
        assert!(self.deposited_collateral >= self.minimum_deposit);
    }

    fn assert_stages(&self, stages: &[Stage]) {
        assert!(stages.contains(&self.stage))
    }

    fn assert_stage(&self, stage: Stage) {
        self.assert_stages(&[stage]);
    }

    fn assert_trading_allowed(&self) {
        self.assert_stage(Stage::Open);
        assert!(env::block_timestamp() < self.end_time);
    }

    fn assert_finalized(&self) {
        self.assert_stages(&[
            Stage::Finalized(Finalization::Resolved),
            Stage::Finalized(Finalization::Invalid),
        ])
    }
}
