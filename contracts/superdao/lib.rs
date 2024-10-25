#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod superdao {
    use ink::{
        prelude::vec::Vec,
        xcm::prelude::*,
        storage::{Mapping},
        scale::{Encode, Decode, Output},
        env::{CallFlags, call::{build_call, ExecutionInput}},
    };

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

    // TODO: move me to a better place
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
        Chain(ChainCall)
    }

    #[derive(Clone)]
    #[cfg_attr(
        feature = "std",
        derive(Debug, PartialEq, Eq, ink::storage::traits::StorageLayout)
    )]
    #[ink::scale_derive(Encode, Decode, TypeInfo)]
    pub struct Proposal {
        call: Call,
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
        pub fn create_proposal(&mut self, call: Call) {
            // TODO: return error
            self.ensure_member();

            let proposal = Proposal {
                call,
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
        pub fn resolve_proposal(&mut self, prop_id: u32) {
            self.ensure_proposal_exists(prop_id);

            let proposal = self.proposals.take(prop_id).expect("Proposal existence confirmed above; qed");

            assert!(self.env().block_number() >= proposal.voting_period_end, "Proposal not ready to execute");

            let votes = self.votes.take(prop_id).expect("No votes yet");

            let total_ayes = votes.iter().filter(|(_, vote)| vote == &1).count() as u8;

            if total_ayes >= self.vote_threshold {
                let result = self.dispatch_call(proposal.call);
            }
        }

        #[cfg(test)]
        fn dispatch_call(&self, call: Call) -> Result<(), Error> { Ok(()) }
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
                            ExecutionInput::new(call.selector.into()).push_arg(CallInput(&call.input)),
                        )
                        .returns::<()>()
                        .try_invoke();
                    assert!(result.is_ok(), "Contract Call failed");
                },
                Call::Chain(call) => {
                    let dest = call.get_dest();
                    let msg = call.get_msg();

                    // TODO: proper error handling
                    // use xcm_execute if dest is local chain, otherwise xcm_send
                    let was_success = if dest == Location::here() {
                        self.env()
                            .xcm_execute(&VersionedXcm::V4(msg)).is_ok()
                    } else {
                        self.env().xcm_send(
                            &VersionedLocation::V4(dest),
                            &VersionedXcm::V4(msg),
                        ).is_ok()
                    };

                    assert!(was_success, "XCM Call failed");
                }
            }
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
        use super::*;

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
        fn create_contract_proposal_works() {
            let mut superdao = Superdao::new();
            let accounts = ink::env::test::default_accounts::<Environment>();
            let call = Call::Contract(ContractCall {
                callee: accounts.alice,
                selector: [0; 4],
                input: vec![],
                transferred_value: 0,
                ref_time_limit: 0,
                allow_reentry: false,
            });

            superdao.register_member(accounts.alice);
            superdao.create_proposal(call.clone());
            assert_eq!(superdao.proposals.get(superdao.next_id-1), Some(Proposal {
                call,
                voting_period_end: 10
            }));
        }

        #[ink::test]
        fn create_chain_proposal_works() {
            let mut superdao = Superdao::new();
            let accounts = ink::env::test::default_accounts::<Environment>();
            let location = Location::here();
            let msg: Xcm<()> = Xcm::new();
            let call = Call::Chain(ChainCall::new(&location, &msg));

            superdao.register_member(accounts.alice);
            superdao.create_proposal(call.clone());
            assert_eq!(superdao.proposals.get(superdao.next_id-1), Some(Proposal {
                call,
                voting_period_end: 10
            }));
        }

        #[ink::test]
        fn submit_vote_works() {
            let mut superdao = Superdao::new();
            let accounts = ink::env::test::default_accounts::<Environment>();
            let call = Call::Contract(ContractCall {
                callee: accounts.alice,
                selector: [0; 4],
                input: vec![],
                transferred_value: 0,
                ref_time_limit: 0,
                allow_reentry: false,
            });

            superdao.register_member(accounts.alice);
            superdao.create_proposal(call);

            superdao.submit_vote(superdao.next_id-1, 1);

            assert_eq!(superdao.votes.get(superdao.next_id-1), Some(vec![(accounts.alice, 1)]));
        }

        // TODO: write this test with e2e tests
        #[ink::test]
        fn resolve_proposal_works() {
            let mut superdao = Superdao::new();
            let accounts = ink::env::test::default_accounts::<Environment>();
            let call = Call::Contract(ContractCall {
                callee: accounts.alice,
                selector: [0; 4],
                input: vec![],
                transferred_value: 0,
                ref_time_limit: 0,
                allow_reentry: false,
            });

            superdao.register_member(accounts.alice);
            superdao.create_proposal(call);
            superdao.submit_vote(superdao.next_id-1, 1);
            for _ in 0..10 {
                ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            }
            superdao.resolve_proposal(superdao.next_id-1);
        }

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
}
