#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod superdao {
    use ink::{
        prelude::vec::Vec,
        storage::{Mapping},
    };

    #[derive(Clone, Default)]
    #[cfg_attr(
        feature = "std",
        derive(Debug, PartialEq, Eq, ink::storage::traits::StorageLayout)
    )]
    #[ink::scale_derive(Encode, Decode, TypeInfo)]
    pub struct Proposal {
        call: u8,
        voting_period_end: BlockNumber,
    }

    /// Errors that can occur upon calling this contract.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[ink::scale_derive(Encode, Decode, TypeInfo)]
    pub enum Error {
        /// Returned if the call failed.
        DispatchFailed,
    }

    //- members: StorageVec< AccountId > //contracts
    // - proposals: Mapping < id, Proposal >
    // - votes: Mapping <prop_id, votes>
    // - vote_threshold: u8
    #[ink(storage)]
    #[derive(Default)]
    pub struct Superdao {
        members: Vec<AccountId>,
        proposals: Mapping < u32, Proposal >,
        votes: Mapping < u32, Vec<(AccountId, u8)> >,
        vote_threshold: u8,
        next_id: u32,
    }

    impl Superdao {
        #[ink(constructor)]
        pub fn new() -> Self {
            Default::default()
        }


        #[ink(message)]
        pub fn register_member(&mut self, member: AccountId) {
            self.members.push(member);
        }

        #[ink(message)]
        pub fn deregister_member(&mut self, member: AccountId) {
            self.members.retain(|&x| x != member);
        }

        #[ink(message)]
        pub fn create_proposal(&mut self, proposal: u8) {
            // TODO: return error
            self.ensure_member();

            let proposal = Proposal {
                call: proposal,
                voting_period_end: self.env().block_number() + 10
            };

            self.proposals.insert(self.next_id, &proposal);
            self.next_id += 1;

            // TODO: event!
        }

        // TODO: vote enum type!
        #[ink(message)]
        pub fn submit_vote(&mut self, prop_id: u32, vote: u8) {
            self.ensure_member();
            self.ensure_proposal_exists(prop_id);

            let mut votes = self.votes.get(&prop_id).unwrap_or_default();
            let maybe_vote = self.find_vote(&votes);

            match maybe_vote {
                Some(index) => {
                   votes[index].1 = vote;
                },
                None => {
                    votes.push((self.env().caller(), vote));
                }
            }

            self.votes.insert(prop_id, &votes);
        }

        #[ink(message)]
        pub fn resolve_proposal(&self, prop_id: u32) {
            self.ensure_proposal_exists(prop_id);

            let proposal = self.proposals.take(prop_id).expect("Proposal existence confirmed above; qed");

            assert!(self.env().block_number() <= proposal.voting_period_end, "Proposal not ready to execute");

            let votes = self.votes.take(prop_id).expect("Proposal existence confirmed above; qed");

            let total_ayes = votes.iter().filter(|(_, vote)| vote == &1).count() as u8;

            if total_ayes >= self.vote_threshold {
                let result = self.dispatch_call(proposal.call);
            }
        }

        fn dispatch_call(&self, call: u8) -> Result<(), Error> {
            Ok(())
        }

        fn ensure_member(&self) {
            assert!(self.members.contains(&self.env().caller()), "Not a member");
        }

        fn ensure_proposal_exists(&self, prop_id: u32) {
            assert!(self.proposals.contains(prop_id), "Proposal does not exist");
        }

        fn find_vote(&self, votes: &Vec<(AccountId, u8)>) -> Option<usize> {
            votes.iter().position(|(x, _)| x == &self.env().caller())
        }
    }

    #[cfg(test)]
    mod tests {
        /// Imports all the definitions from the outer scope so we can use them here.
        use super::*;

        /// We test if the default constructor does its job.
        #[ink::test]
        fn default_works() {
            let superdao = Superdao::new();
            assert_eq!(superdao.members.len(), 0);
            assert_eq!(superdao.vote_threshold, 0);
            assert_eq!(superdao.next_id, 0);
        }

        #[ink::test]
        fn register_member_works() {
            let mut superdao = Superdao::new();
            let accounts = ink::env::test::default_accounts::<Environment>();

            superdao.register_member(accounts.alice);
            superdao.register_member(accounts.bob);
            assert_eq!(superdao.members.len(), 2);
        }

        #[ink::test]
        fn deregister_member_works() {
            let mut superdao = Superdao::new();
            let accounts = ink::env::test::default_accounts::<Environment>();

            superdao.register_member(accounts.alice);
            superdao.register_member(accounts.bob);

            superdao.deregister_member(accounts.alice);
            assert_eq!(superdao.members.len(), 1);
            superdao.deregister_member(accounts.bob);
            assert_eq!(superdao.members.len(), 0);
        }

        #[ink::test]
        fn create_proposal_works() {
            let mut superdao = Superdao::new();
            let accounts = ink::env::test::default_accounts::<Environment>();

            superdao.register_member(accounts.alice);
            superdao.create_proposal(1);
            assert_eq!(superdao.proposals.get(superdao.next_id-1), Some(Proposal {
                call: 1,
                voting_period_end: 10
            }));
        }

        #[ink::test]
        fn submit_vote_works() {
            let mut superdao = Superdao::new();
            let accounts = ink::env::test::default_accounts::<Environment>();

            superdao.register_member(accounts.alice);
            superdao.create_proposal(1);

            superdao.submit_vote(superdao.next_id-1, 1);

            assert_eq!(superdao.votes.get(superdao.next_id-1), Some(vec![(accounts.alice, 1)]));
        }

        #[ink::test]
        fn resolve_proposal_works() {
            let mut superdao = Superdao::new();
            let accounts = ink::env::test::default_accounts::<Environment>();

            superdao.register_member(accounts.alice);
            superdao.create_proposal(1);
            superdao.submit_vote(superdao.next_id-1, 1);

            superdao.resolve_proposal(superdao.next_id-1);
        }
    }
}
