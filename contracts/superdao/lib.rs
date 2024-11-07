#![cfg_attr(not(feature = "std"), no_std, no_main)]

// TODO tracker
// - current member implementation is insecure -> can spin up many contracts and take over voting
// - need failing tests
// - need getters - mostly done
// - e2e tests
// - limit registering to contract addresses only <- if gov token, maybe not
// - emit events

#[ink::contract]
mod superdao {
    use ink::codegen::Env;
    use ink::{
        env::{
            call::{build_call, ExecutionInput},
            CallFlags,
        },
        prelude::vec::Vec,
        scale::{Decode, Encode, Output},
        storage::Mapping,
        xcm::prelude::*,
    };
    use superdao_traits::{Call, ChainCall, ContractCall, Proposal, SuperDao, SuperDaoQuery, Vote};

    /// A wrapper that allows us to encode a blob of bytes.
    ///
    /// We use this to pass the set of untyped (bytes) parameters to the `CallBuilder`.
    #[derive(Clone)]
    struct CallInput<'a>(&'a [u8]);

    impl<'a> ink::scale::Encode for CallInput<'a> {
        fn encode_to<T: Output + ?Sized>(&self, dest: &mut T) {
            dest.write(self.0);
        }
    }

    #[ink(storage)]
    #[derive(Default)]
    pub struct Superdao {
        members: Vec<AccountId>,
        proposals: Mapping<u32, Proposal>,
        active_proposals: Vec<u32>,
        votes: Mapping<u32, Vec<(AccountId, Vote)>>,
        next_id: u32,
        vote_threshold: u8,
        voting_period: BlockNumber,
    }

    impl Superdao {
        #[ink(constructor, payable)]
        pub fn new(vote_threshold: u8, voting_period: BlockNumber) -> Self {
            Self {
                members: Vec::new(),
                proposals: Mapping::new(),
                active_proposals: Vec::new(),
                votes: Mapping::new(),
                next_id: 0,
                vote_threshold,
                voting_period,
            }
        }

        #[ink(constructor, payable)]
        pub fn default() -> Self {
            Default::default()
        }

        #[ink(message)]
        pub fn resolve_proposal(&mut self, prop_id: u32) {
            assert!(
                self.ensure_proposal_exists(prop_id).is_ok(),
                "Proposal does not exist."
            );

            let proposal = self
                .proposals
                .take(prop_id)
                .expect("Proposal existence confirmed above; qed");

            self.active_proposals.retain(|&x| x != prop_id);

            assert!(
                self.env().block_number() >= proposal.voting_period_end,
                "Proposal not ready to execute"
            );

            let votes = self.votes.take(prop_id).expect("No votes yet");

            let total_ayes = votes.iter().filter(|(_, vote)| vote == &Vote::Aye).count() as u8;

            if total_ayes >= self.vote_threshold {
                let result = self.dispatch_call(proposal.call);
            }
        }

        #[cfg(test)]
        fn dispatch_call(&self, call: Call) -> Result<(), Error> {
            Ok(())
        }
        #[cfg(not(test))]
        fn dispatch_call(&self, call: Call) -> Result<(), Error> {
            // TODO: revisit value transferred
            match call {
                Call::Contract(call) => {
                    // source: https://github.com/use-ink/ink-examples/blob/main/multisig/lib.rs#L541
                    let call_flags = if call.allow_reentry {
                        CallFlags::ALLOW_REENTRY
                    } else {
                        CallFlags::empty()
                    };

                    let result = build_call::<<Self as ::ink::env::ContractEnv>::Env>()
                        .call(call.callee)
                        .ref_time_limit(call.ref_time_limit)
                        .transferred_value(call.transferred_value)
                        .call_flags(call_flags)
                        .exec_input(
                            ExecutionInput::new(call.selector.into())
                                .push_arg(CallInput(&call.input)),
                        )
                        .returns::<()>()
                        .try_invoke();
                    assert!(result.is_ok(), "Contract Call failed");
                }
                Call::Chain(call) => {
                    let dest = call.get_dest();
                    let msg = call.get_msg();

                    // TODO: proper error handling
                    // use xcm_execute if dest is local chain, otherwise xcm_send
                    let was_success = if dest == Location::here() {
                        self.env().xcm_execute(&VersionedXcm::V4(msg)).is_ok()
                    } else {
                        self.env()
                            .xcm_send(&VersionedLocation::V4(dest), &VersionedXcm::V4(msg))
                            .is_ok()
                    };

                    assert!(was_success, "XCM Call failed");
                }
            }
            Ok(())
        }

        fn ensure_member(&self) -> Result<(), Error> {
            if !self.is_member() {
                return Err(Error::NotMember);
            }
            Ok(())
        }

        fn ensure_proposal_exists(&self, prop_id: u32) -> Result<(), Error> {
            if !self.proposals.contains(prop_id) {
                return Err(Error::ProposalNotFound);
            }
            Ok(())
        }

        fn find_vote(&self, votes: &Vec<(AccountId, Vote)>) -> Option<usize> {
            votes.iter().position(|(x, _)| x == &self.env().caller())
        }
    }

    impl SuperDao for Superdao {
        #[ink(message)]
        fn register_member(&mut self) -> Result<(), Error> {
            if self.is_member() {
                return Err(Error::AlreadyMember);
            }
            self.members.push(self.env().caller());
            Ok(())
        }

        #[ink(message)]
        fn deregister_member(&mut self) {
            let caller = self.env().caller();
            self.members.retain(|&x| x != caller);
        }

        #[ink(message)]
        fn create_proposal(&mut self, call: Call) -> Result<(), Error> {
            self.ensure_member()?;

            let proposal = Proposal {
                call,
                voting_period_end: self.env().block_number().saturating_add(self.voting_period),
            };

            self.proposals.insert(self.next_id, &proposal);
            self.active_proposals.push(self.next_id);
            self.next_id = self.next_id.saturating_add(1);

            Ok(())
            // TODO: event!
        }

        // TODO: vote enum type!
        #[ink(message)]
        fn vote(&mut self, prop_id: u32, vote: Vote) -> Result<(), Error> {
            self.ensure_member()?;
            self.ensure_proposal_exists(prop_id)?;

            let mut votes = self.votes.get(&prop_id).unwrap_or_default();
            let maybe_vote = self.find_vote(&votes);

            match maybe_vote {
                Some(index) => {
                    votes[index].1 = vote;
                }
                None => {
                    votes.push((self.env().caller(), vote));
                }
            }

            self.votes.insert(prop_id, &votes);
            Ok(())
        }
    }

    impl SuperDaoQuery for Superdao {
        #[ink(message)]
        fn get_members(&self) -> Vec<AccountId> {
            self.members.clone()
        }

        #[ink(message)]
        fn is_member(&self) -> bool {
            self.members.contains(&self.env().caller())
        }

        #[ink(message)]
        fn get_proposal(&self, index: u32) -> Option<Proposal> {
            self.proposals.get(index)
        }

        #[ink(message)]
        fn get_proposals(&self) -> Vec<Proposal> {
            self.active_proposals
                .iter()
                .map(|&x| {
                    self.proposals
                        .get(x)
                        .expect("If prop_id is present, proposal exists.")
                })
                .collect()
        }

        #[ink(message)]
        fn get_votes(&self, proposal_id: u32) -> Vec<(AccountId, Vote)> {
            self.votes.get(proposal_id).unwrap_or_default()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[ink::test]
        fn new_works() {
            let superdao = Superdao::new(5, 4);
            assert_eq!(superdao.members.len(), 0);
            assert_eq!(superdao.active_proposals.len(), 0);
            assert_eq!(superdao.next_id, 0);
            assert_eq!(superdao.vote_threshold, 5);
            assert_eq!(superdao.voting_period, 4);
        }

        #[ink::test]
        fn default_works() {
            let superdao = Superdao::default();
            assert_eq!(superdao.members.len(), 0);
            assert_eq!(superdao.active_proposals.len(), 0);
            assert_eq!(superdao.next_id, 0);
            assert_eq!(superdao.vote_threshold, 0);
            assert_eq!(superdao.voting_period, 0);
        }

        #[ink::test]
        fn register_member_works() {
            let mut superdao = Superdao::default();
            let accounts = ink::env::test::default_accounts::<Environment>();

            superdao.register_member();
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.bob);
            superdao.register_member();
            assert_eq!(superdao.members.len(), 2);
        }

        #[ink::test]
        fn deregister_member_works() {
            let mut superdao = Superdao::default();
            let accounts = ink::env::test::default_accounts::<Environment>();

            superdao.register_member();
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.bob);
            superdao.register_member();

            superdao.deregister_member();
            assert_eq!(superdao.members.len(), 1);
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.alice);
            superdao.deregister_member();
            assert_eq!(superdao.members.len(), 0);
        }

        #[ink::test]
        fn create_contract_proposal_works() {
            let mut superdao = Superdao::default();
            let accounts = ink::env::test::default_accounts::<Environment>();
            let call = Call::Contract(ContractCall {
                callee: accounts.alice,
                selector: [0; 4],
                input: vec![],
                transferred_value: 0,
                ref_time_limit: 0,
                allow_reentry: false,
            });

            superdao.register_member();
            superdao.create_proposal(call.clone());
            assert_eq!(
                superdao.proposals.get(superdao.next_id - 1),
                Some(Proposal {
                    call,
                    voting_period_end: 0
                })
            );
            assert_eq!(superdao.active_proposals.len(), 1);
        }

        #[ink::test]
        fn create_chain_proposal_works() {
            let mut superdao = Superdao::default();
            let accounts = ink::env::test::default_accounts::<Environment>();
            let location = Location::here();
            let msg: Xcm<()> = Xcm::new();
            let call = Call::Chain(ChainCall::new(&location, &msg));

            superdao.register_member();
            superdao.create_proposal(call.clone());
            assert_eq!(
                superdao.proposals.get(superdao.next_id - 1),
                Some(Proposal {
                    call,
                    voting_period_end: 0
                })
            );
            assert_eq!(superdao.active_proposals.len(), 1);
        }

        #[ink::test]
        fn vote_works() {
            let mut superdao = Superdao::default();
            let accounts = ink::env::test::default_accounts::<Environment>();
            let call = Call::Contract(ContractCall {
                callee: accounts.alice,
                selector: [0; 4],
                input: vec![],
                transferred_value: 0,
                ref_time_limit: 0,
                allow_reentry: false,
            });

            superdao.register_member();
            superdao.create_proposal(call);

            superdao.vote(superdao.next_id - 1, Vote::Aye);

            assert_eq!(
                superdao.votes.get(superdao.next_id - 1),
                Some(vec![(accounts.alice, Vote::Aye)])
            );
        }

        // TODO: write this test with e2e tests
        #[ink::test]
        fn resolve_proposal_works() {
            let mut superdao = Superdao::default();
            let accounts = ink::env::test::default_accounts::<Environment>();
            let call = Call::Contract(ContractCall {
                callee: accounts.alice,
                selector: [0; 4],
                input: vec![],
                transferred_value: 0,
                ref_time_limit: 0,
                allow_reentry: false,
            });

            superdao.register_member();
            superdao.create_proposal(call);
            superdao.vote(superdao.next_id - 1, Vote::Nay);
            for _ in 0..10 {
                ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            }
            superdao.resolve_proposal(superdao.next_id - 1);
            assert_eq!(superdao.proposals.get(superdao.next_id - 1), None);
            assert_eq!(superdao.active_proposals.len(), 0);
        }

        mod super_dao_query {
            use super::*;

            #[ink::test]
            fn get_members_works() {
                let mut superdao = Superdao::default();
                let accounts = ink::env::test::default_accounts::<Environment>();

                superdao.register_member();
                ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.bob);
                superdao.register_member();

                assert_eq!(superdao.get_members(), vec![accounts.alice, accounts.bob]);
            }

            #[ink::test]
            fn is_member_works() {
                let mut superdao = Superdao::default();
                let accounts = ink::env::test::default_accounts::<Environment>();

                superdao.register_member();
                assert!(superdao.is_member());

                ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.bob);
                assert!(!superdao.is_member());
            }

            #[ink::test]
            fn get_proposal_works() {
                let mut superdao = Superdao::default();
                let accounts = ink::env::test::default_accounts::<Environment>();
                let call = Call::Contract(ContractCall {
                    callee: accounts.alice,
                    selector: [0; 4],
                    input: vec![],
                    transferred_value: 0,
                    ref_time_limit: 0,
                    allow_reentry: false,
                });

                superdao.register_member();
                superdao.create_proposal(call.clone());

                assert_eq!(
                    superdao.get_proposal(superdao.next_id - 1),
                    Some(Proposal {
                        call,
                        voting_period_end: 0
                    })
                );
            }

            #[ink::test]
            fn get_proposals_works() {
                let mut superdao = Superdao::default();
                let accounts = ink::env::test::default_accounts::<Environment>();
                let call = Call::Contract(ContractCall {
                    callee: accounts.alice,
                    selector: [0; 4],
                    input: vec![],
                    transferred_value: 0,
                    ref_time_limit: 0,
                    allow_reentry: false,
                });

                superdao.register_member();
                superdao.create_proposal(call.clone());

                assert_eq!(
                    superdao.get_proposals(),
                    vec![Proposal {
                        call,
                        voting_period_end: 0
                    }]
                );
            }

            #[ink::test]
            fn get_votes_works() {
                let mut superdao = Superdao::default();
                let accounts = ink::env::test::default_accounts::<Environment>();
                let call = Call::Contract(ContractCall {
                    callee: accounts.alice,
                    selector: [0; 4],
                    input: vec![],
                    transferred_value: 0,
                    ref_time_limit: 0,
                    allow_reentry: false,
                });

                superdao.register_member();
                superdao.create_proposal(call.clone());
                superdao.vote(superdao.next_id - 1, Vote::Aye);

                assert_eq!(
                    superdao.get_votes(superdao.next_id - 1),
                    vec![(accounts.alice, Vote::Aye)]
                );
            }
        }

        #[ink::test]
        fn xcm_encoded_calls_helper() {
            let location = Location::here();

            let accounts = ink::env::test::default_accounts::<Environment>();

            let value: Balance = 10000000000;
            let asset: Asset = (Location::parent(), value).into();
            let beneficiary = AccountId32 {
                network: None,
                id: *accounts.alice.as_ref(),
            };

            let msg: Xcm<()> = Xcm::builder()
                .withdraw_asset(asset.clone().into())
                .buy_execution(asset.clone(), Unlimited)
                .deposit_asset(asset.into(), beneficiary.into())
                .build();

            let chain_call = ChainCall::new(&location, &msg);

            ink::env::debug_println!("dest: {:?}", hex::encode(chain_call.get_encoded_dest()));
            ink::env::debug_println!("msg: {:?}", hex::encode(chain_call.get_encoded_msg()));
        }
    }
}
