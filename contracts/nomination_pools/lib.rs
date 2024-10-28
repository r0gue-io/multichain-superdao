#![cfg_attr(not(feature = "std"), no_std, no_main)]

use pop_api::{incentives as incentives_api, messaging as api, primitives::Era, StatusCode};

pub type ApiResult<T> = core::result::Result<T, StatusCode>;

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

#[ink::contract]
mod nomination_pools {
    use super::*;
    use ink::{
        codegen::TraitCallBuilder,
        env::{
            call::{build_call, build_create, ExecutionInput, Selector},
            DefaultEnvironment,
        },
        prelude::vec::Vec,
        scale::Decode,
        storage::Mapping,
        xcm::prelude::*,
    };
    use superdao::SuperdaoRef;

    #[derive(Clone)]
    #[cfg_attr(
        feature = "std",
        derive(Debug, PartialEq, Eq, ink::storage::traits::StorageLayout)
    )]
    #[ink::scale_derive(Encode, Decode, TypeInfo)]
    pub struct ValidatorsProposal {
        pub encoded_extrinsic: Vec<u8>,
        pub ref_time: u64,
        pub proof_size: u64,
    }

    #[ink(storage)]
    pub struct NominationPools {
        validators_proposal: Mapping<u32, ValidatorsProposal>,
        votes: Mapping<u32, Vec<(AccountId, Vote)>>,
        superdao: SuperdaoRef,
        next_id: u32,
    }

    #[ink::event]
    pub struct ValidatorsSuggested {
        #[ink(topic)]
        pub who: AccountId,
        pub proposal_id: u32,
    }

    #[ink::event]
    pub struct VoteCast {
        #[ink(topic)]
        pub who: AccountId,
        #[ink(topic)]
        pub proposal_id: u32,
        pub vote: Vote,
    }

    #[ink::event]
    pub struct ValidatorsChanged {
        #[ink(topic)]
        pub proposal_id: u32,
    }

    #[derive(Debug, PartialEq, Eq, Copy, Clone)]
    #[ink::scale_derive(Encode, Decode, TypeInfo)]
    pub enum Error {
        ValidatorsProposalNonExistent,
    }

    impl NominationPools {
        /// Constructor that initializes the superdao contract and some values specific to this contract.
        #[ink(constructor, payable)]
        pub fn new(superdao_contract_code_hash: Hash) -> Self {
            //pub fn new() -> Self {
            let superdao: SuperdaoRef = build_create::<SuperdaoRef>()
                .code_hash(superdao_contract_code_hash)
                .endowment(0)
                .exec_input(
                    ExecutionInput::new(Selector::new(ink::selector_bytes!("new")))
                        .push_arg(2)
                        .push_arg(0),
                )
                .salt_bytes(&[0xDE, 0xAD, 0xBE, 0xEF])
                .returns::<SuperdaoRef>()
                .instantiate();
            Self {
                validators_proposal: Mapping::new(),
                votes: Mapping::new(),
                superdao,
                next_id: 0,
            }
        }

        #[ink(message)]
        pub fn register_contract_incentives(&mut self) -> ApiResult<()> {
            let address = self.env().account_id();
            incentives_api::register(address)
        }

        #[ink(message)]
        pub fn claim_rewards_incentives(&mut self, era: Era) -> ApiResult<()> {
            incentives_api::claim(era)
        }

        // Call in the realy chain to nominationPools - nominate (poolid, validators)
        #[ink(message, payable)]
        pub fn suggest_validators(
            &mut self,
            encoded_extrinsic: Vec<u8>,
            ref_time: u64,
            proof_size: u64,
        ) -> Result<(), Error> {
            self.superdao.ensure_member();
            let proposal = ValidatorsProposal {
                encoded_extrinsic,
                ref_time,
                proof_size,
            };
            self.validators_proposal.insert(self.next_id, &proposal);
            self.env().emit_event(ValidatorsSuggested {
                who: self.env().caller(),
                proposal_id: self.next_id,
            });
            self.next_id = self.next_id.saturating_add(1);
            Ok(())
        }

        #[ink(message, payable)]
        pub fn vote_validators(&mut self, id: u32, vote: Vote) -> Result<(), Error> {
            self.superdao.ensure_member();
            let proposal = self
                .validators_proposal
                .get(id)
                .ok_or(Error::ValidatorsProposalNonExistent)?;
            // Vote on the proposal
            let mut votes = self.votes.get(&id).unwrap_or_default();
            let maybe_vote = votes.iter().position(|(x, _)| x == &self.env().caller());
            match maybe_vote {
                Some(index) => {
                    votes[index].1 = vote.clone();
                }
                None => {
                    votes.push((self.env().caller(), vote.clone()));
                }
            }
            self.votes.insert(id, &votes);
            self.env().emit_event(VoteCast {
                who: self.env().caller(),
                proposal_id: id,
                vote,
            });
            // Check if we have enough votes to execute the proposal
            let total_ayes = votes.iter().filter(|(_, vote)| vote == &Vote::Aye).count() as u8;
            if total_ayes >= self.superdao.get_vote_threshold() {
                let result = self.dispatch_call(proposal);
                self.env().emit_event(ValidatorsChanged { proposal_id: id });
                self.validators_proposal.remove(id);
                self.votes.remove(id);
            }
            // TODO: If total_nayes is > threshold, remove proposal
            Ok(())
        }

        fn dispatch_call(&mut self, proposal: ValidatorsProposal) -> Result<(), Error> {
            let encoded_extrinsic: Vec<u8> = proposal.encoded_extrinsic;
            let weight = Weight::from_parts(proposal.ref_time, proposal.proof_size);

            let asset: Asset = (Here, self.env().transferred_value()).into();
            let dest = Location::parent().into_versioned();

            let message: Xcm<()> = Xcm::builder()
                .withdraw_asset(asset.clone().into())
                .buy_execution(asset.clone(), Unlimited)
                .transact(
                    OriginKind::SovereignAccount,
                    weight,
                    encoded_extrinsic.into(),
                )
                .build();

            api::xcm::send(&dest, &VersionedXcm::V4(message)).unwrap();
            Ok(())
        }

        /// Returns the current votes of a proposal.
        #[ink(message, payable)]
        pub fn get_votes(&self, proposal_id: u32) -> Option<Vec<(AccountId, Vote)>> {
            self.votes.get(proposal_id)
        }

        /// Returns the proposal call.
        #[ink(message, payable)]
        pub fn get_new_validators_proposals(&self, proposal_id: u32) -> Option<ValidatorsProposal> {
            self.validators_proposal.get(proposal_id)
        }

        // TODO: Remove, just for testing
        #[ink(message, payable)]
        pub fn test_xcm_call(
            &mut self,
            encoded_extrinsic: Vec<u8>,
            ref_time: u64,
            proof_size: u64,
        ) -> Result<(), Error> {
            let weight = Weight::from_parts(ref_time, proof_size);
            let asset: Asset = (Here, self.env().transferred_value()).into();

            let dest = Location::parent().into_versioned();

            let message: Xcm<()> = Xcm::builder()
                .withdraw_asset(asset.clone().into())
                .buy_execution(asset.clone(), Unlimited)
                .transact(
                    OriginKind::SovereignAccount,
                    weight,
                    encoded_extrinsic.into(),
                )
                .build();
            //let _ = self.env().xcm_send(&dest, &VersionedXcm::V4(message));
            api::xcm::send(&dest, &VersionedXcm::V4(message)).unwrap();
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        // #[ink::test]
        // fn new_works() {
        //     let superdao = Superdao::new(2, 0);
        //     let hash = superdao.code_hash();
        //     assert_eq!(hash, "hash");
        //     let contract = NominationPools::new(hash);
        //     assert_eq!(contract.next_id, 0);
        // }
    }
}
