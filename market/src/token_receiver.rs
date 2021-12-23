use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::{log, serde_json, PromiseOrValue};

use crate::instructions::*;
use crate::*;

#[near_bindgen]
impl FungibleTokenReceiver for Contract {
    fn ft_on_transfer(
        &mut self,
        sender_id: ValidAccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        let sender: AccountId = sender_id.into();
        let amount: u128 = amount.into();
        let token_id = env::predecessor_account_id();
        let message = serde_json::from_str::<Instruction>(&msg).expect(errors::INVALID_MESSAGE);
        match message {
            Instruction::Buy(ix) => self.buy(&sender, &token_id, amount, ix),
            Instruction::InitialDeposit(ix) => self.deposit(&sender, &token_id, amount, ix),
            _ => panic!("Not implemented"),
        }
    }
}
