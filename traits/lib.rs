#![cfg_attr(not(feature = "std"), no_std, no_main)]

use ink::{
    env::{DefaultEnvironment, Environment},
    prelude::{string::String, vec::Vec},
    primitives::AccountId,
    scale::{Decode, Encode, Output},
    xcm::prelude::*,
};

type Balance = <DefaultEnvironment as Environment>::Balance;
type BlockNumber = <DefaultEnvironment as Environment>::BlockNumber;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[ink::scale_derive(Encode, Decode, TypeInfo)]
pub enum Error {
    DispatchFailed,
    AlreadyMember,
    NotMember,
    ProposalNotFound,
}

#[derive(Clone, PartialEq)]
#[cfg_attr(
    feature = "std",
    derive(Debug, Eq, ink::storage::traits::StorageLayout)
)]
#[ink::scale_derive(Encode, Decode, TypeInfo)]
pub enum Vote {
    Aye,
    Nay,
}

#[derive(Clone)]
#[cfg_attr(
    feature = "std",
    derive(Debug, PartialEq, Eq, ink::storage::traits::StorageLayout)
)]
#[ink::scale_derive(Encode, Decode, TypeInfo)]
pub struct Proposal {
    pub call: Call,
    pub voting_period_end: BlockNumber,
}

#[ink::trait_definition]
pub trait SuperDao {
    #[ink(message)]
    fn register_member(&mut self) -> Result<(), Error>;

    #[ink(message)]
    fn deregister_member(&mut self);

    #[ink(message)]
    fn create_proposal(&mut self, call: Call) -> Result<u32, Error>;

    #[ink(message)]
    fn vote(&mut self, proposal_id: u32, vote: Vote) -> Result<(), Error>;
}

#[ink::trait_definition]
pub trait SuperDaoQuery {
    #[ink(message)]
    fn get_members(&self) -> Vec<AccountId>;

    #[ink(message)]
    fn is_member(&self) -> bool;

    #[ink(message)]
    fn get_proposal(&self, index: u32) -> Option<Proposal>;

    #[ink(message)]
    fn get_proposals(&self) -> Vec<(u32, Proposal)>;

    #[ink(message)]
    fn get_vote_threshold(&self) -> u8;

    #[ink(message)]
    fn get_voting_period(&self) -> BlockNumber;

    #[ink(message)]
    fn get_active_proposals(&self) -> Vec<u32>;

    #[ink(message)]
    fn get_votes(&self, proposal_id: u32) -> Vec<(AccountId, Vote)>;
}

#[derive(Clone)]
#[cfg_attr(
    feature = "std",
    derive(Debug, PartialEq, Eq, ink::storage::traits::StorageLayout)
)]
#[ink::scale_derive(Encode, Decode, TypeInfo)]
// src: https://github.com/use-ink/ink-examples/blob/main/multisig/lib.rs#L119
pub struct ContractCall {
    /// The `AccountId` of the contract that is called in this transaction.
    pub callee: AccountId,
    /// The selector bytes that identifies the function of the callee that should be
    /// called.
    pub selector: [u8; 4],
    /// The SCALE encoded parameters that are passed to the called function.
    pub input: Vec<u8>,
    /// The amount of chain balance that is transferred to the callee.
    pub transferred_value: Balance,
    /// Gas limit for the execution of the call.
    pub ref_time_limit: u64,
    /// If set to true the transaction will be allowed to re-enter the multisig
    /// contract. Re-entrancy can lead to vulnerabilities. Use at your own
    /// risk.
    pub allow_reentry: bool,
}

#[derive(Clone)]
#[cfg_attr(
    feature = "std",
    derive(Debug, PartialEq, Eq, ink::storage::traits::StorageLayout)
)]
#[ink::scale_derive(Encode, Decode, TypeInfo)]
pub struct ChainCall {
    // encoded XCM `Location`
    dest: Vec<u8>,
    // encoded XCM `Message`
    msg: Vec<u8>,
}

impl ChainCall {
    pub fn new(dest: &Location, msg: &Xcm<()>) -> Self {
        Self {
            dest: dest.encode(),
            msg: msg.encode(),
        }
    }

    pub fn get_dest(&self) -> Location {
        Location::decode(&mut &self.dest[..]).expect("dest should have valid encoding.")
    }

    pub fn get_msg(&self) -> Xcm<()> {
        Xcm::decode(&mut &self.msg[..]).expect("msg should have valid encoding.")
    }

    pub fn get_encoded_dest(&self) -> Vec<u8> {
        self.dest.clone()
    }

    pub fn get_encoded_msg(&self) -> Vec<u8> {
        self.msg.clone()
    }
}

#[derive(Clone)]
#[cfg_attr(
    feature = "std",
    derive(Debug, PartialEq, Eq, ink::storage::traits::StorageLayout)
)]
#[ink::scale_derive(Encode, Decode, TypeInfo)]
pub enum Call {
    Contract(ContractCall),
    Chain(ChainCall),
}

// tests
#[cfg(test)]
mod chain_call {
    use super::*;
    #[ink::test]
    fn new_works() {
        let location = Location::here();
        let msg: Xcm<()> = Xcm::new();
        let chain_call = ChainCall::new(&location, &msg);

        assert_eq!(chain_call.get_dest(), location);
        assert_eq!(chain_call.get_msg(), msg);
        assert_eq!(&chain_call.get_encoded_dest(), &location.encode());
        assert_eq!(&chain_call.get_encoded_msg(), &msg.encode());
    }
}
