# Nomination Pools (WIP)
Update the list of validators to nominate from the pool managed by the DAO.

## Get Started
1- First we have to deploy the superdao contract
```
cd ../superdao
pop build --release
pop up contract 2 0
```
Now that superdao contract is deployed, look up the code_hash of the deployed contract using PolkadotJs Apps.
2- Deploy the nomination_pools contract
```
pop build --release
// input the superdaocode_hash as the parameter
pop up contract --args 0xb7e78057babb3ca02c0d5fec7fd19ff2acfd23ce590948b654eb57b792781556
```
3- Register members in the superDao.

4- Create a Nomination Pool in the relay using the AccountId32 of the contract (check in events): nominationPools -> create()
5- Take the pool_id (events) and prepare the extrinsic to change validators: nominationPools -> nominate(poolId, listOfValidators). Get the call data.

6- Register the validators to nominate
```
pop call contract --contract nomination_pool_address (0x47447451e4b2cfd8de9048258412a360a88444a4) --message suggest_nominators --args call_data ref_time proof_size --suri //Alice -x
```
7- Take the proposal_id in the events and vote with other members of the superDao. Example:
```
pop call contract --contract nomination_pool_address --message vote_nominators --args 0 Aye --suri //Bob -x
```

## WIP: Errors.
- Error in cross-contract chain, when initialise:
```
Pre-submission dry-run failed: ModuleError: Revive::ContractTrapped: ["Contract trapped during execution."]
```
- Error in the XCM call.
```
Pre-submission dry-run failed: ModuleError: Revive::DecodingFailed: ["Input passed to a contract API function failed to decode as expected type."]
```
To test comment the superdao instances and call the extrinsic: `test_xcm_call`