#![cfg_attr(not(feature = "std"), no_std, no_main)]

use pop_api::{incentives as incentives_api, primitives::Era, StatusCode};

pub type ApiResult<T> = core::result::Result<T, StatusCode>;

#[ink::contract]
mod nomination_pools {
    use super::*;
    use ink::{prelude::vec::Vec, scale::Decode, storage::Mapping, xcm::prelude::*};
    use superda0_traits::superdao::Vote;
    use superdao::SuperdaoRef;

    #[derive(Clone)]
    #[cfg_attr(
        feature = "std",
        derive(Debug, PartialEq, Eq, ink::storage::traits::StorageLayout)
    )]
    #[ink::scale_derive(Encode, Decode, TypeInfo)]
    pub struct NominatorsProposal {
        pub encoded_extrinsic: Vec<u8>,
        pub ref_time: u64,
        pub proof_size: u64,
    }

    #[ink(storage)]
    pub struct NominationPools {
        nominators_proposal: Mapping<u32, NominatorsProposal>,
        votes: Mapping<u32, Vec<(AccountId, Vote)>>,
        superdao: SuperdaoRef,
        next_id: u32,
    }

    #[ink::event]
    pub struct PoolCreated {
        #[ink(topic)]
        pub hash: bool,
    }

    #[derive(Debug, PartialEq, Eq, Copy, Clone)]
    #[ink::scale_derive(Encode, Decode, TypeInfo)]
    pub enum Error {
        NominatorsProposalNonExistent,
    }

    impl NominationPools {
        /// Constructor that initializes the superdao contract and some values specific to this contract.
        #[ink(constructor, payable)]
        pub fn new(superdao_contract_code_hash: Hash) -> Self {
            let superdao = SuperdaoRef::new(2, 0)
                .code_hash(superdao_contract_code_hash)
                .endowment(0)
                .salt_bytes([0xDE, 0xAD, 0xBE, 0xEF])
                .instantiate();

            Self {
                nominators_proposal: Mapping::new(),
                votes: Mapping::new(),
                superdao,
                next_id: 0,
            }
        }

        #[ink(message)]
        pub fn register_contract_builder_incentives(&mut self) -> ApiResult<()> {
            let address = self.env().account_id();
            incentives_api::register(address)
        }

        #[ink(message)]
        pub fn claim_rewards(&mut self, era: Era) -> ApiResult<()> {
            incentives_api::claim(era)
        }

        #[ink(message, payable)]
        pub fn suggest_nominators(
            &mut self,
            encoded_extrinsic: Vec<u8>,
            ref_time: u64,
            proof_size: u64,
        ) -> Result<(), Error> {
            //self.superdao.ensure_member();
            let proposal = NominatorsProposal {
                encoded_extrinsic,
                ref_time,
                proof_size,
            };
            self.nominators_proposal.insert(self.next_id, &proposal);
            self.next_id = self.next_id.saturating_add(1);
            Ok(())
        }

        #[ink(message, payable)]
        pub fn vote_nominators(&mut self, id: u32, vote: Vote) -> Result<(), Error> {
            //self.superdao.ensure_member();
            assert!(
                self.nominators_proposal.contains(id),
                "Proposal does not exist"
            );
            let proposal = self
                .nominators_proposal
                .get(id)
                .ok_or(Error::NominatorsProposalNonExistent)?;
            let mut votes = self.votes.get(&id).unwrap_or_default();
            let maybe_vote = votes.iter().position(|(x, _)| x == &self.env().caller());
            match maybe_vote {
                Some(index) => {
                    votes[index].1 = vote;
                }
                None => {
                    votes.push((self.env().caller(), vote));
                }
            }

            self.votes.insert(id, &votes);

            // Check if we have enough votes to execute the proposal
            let total_ayes = votes.iter().filter(|(_, vote)| vote == &Vote::Aye).count() as u8;

            //if total_ayes >= self.superdao.get_vote_threshold() {
            let result = self.dispatch_call(proposal);
            self.nominators_proposal.remove(id);
            self.votes.remove(id);
            //}
            Ok(())
        }

        fn dispatch_call(&mut self, proposal: NominatorsProposal) -> Result<(), Error> {
            let encoded_extrinsic = proposal.encoded_extrinsic;
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

            let _ = self.env().xcm_send(&dest, &VersionedXcm::V4(message));
            //api::xcm::execute(&VersionedXcm::V4(message)).unwrap();
            Ok(())
        }

        /// Returns the current votes of a proposal.
        #[ink(message, payable)]
        pub fn get_votes(&self, proposal_id: u32) -> Option<Vec<(AccountId, Vote)>> {
            self.votes.get(proposal_id)
        }

        /// Returns the proposal call.
        #[ink(message, payable)]
        pub fn get_nomination_pools_proposals(
            &self,
            proposal_id: u32,
        ) -> Option<NominatorsProposal> {
            self.nominators_proposal.get(proposal_id)
        }
    }

    #[cfg(test)]
    mod tests {

        use super::*;

        // #[ink::test]
        // fn new_works() {
        //     let superdao = Superdao::new(5, 4);
        //     assert_eq!(superdao.members.len(), 0);
        //     assert_eq!(superdao.active_proposals.len(), 0);
        //     assert_eq!(superdao.next_id, 0);
        //     assert_eq!(superdao.vote_threshold, 5);
        //     assert_eq!(superdao.voting_period, 4);
        // }
    }
}
