#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit="256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
use sp_std::prelude::*;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};
use sp_runtime::{
	DispatchResult,
	ModuleId,
	Percent,
	ApplyExtrinsicResult, generic, create_runtime_str, impl_opaque_keys, MultiSignature,
	transaction_validity::{TransactionValidity, TransactionSource},
};
pub use sp_runtime::traits::{
	ConvertInto,
	BlakeTwo256, Block as BlockT, AccountIdLookup, Verify, IdentifyAccount,

};
use sp_api::impl_runtime_apis;

// XCM imports
use polkadot_parachain::primitives::Sibling;
use xcm::v0::{Junction, MultiLocation, NetworkId, Xcm};
use xcm_builder::{
	AccountId32Aliases, CurrencyAdapter, LocationInverter, ParentIsDefault, RelayChainAsNative,
	SiblingParachainAsNative, SiblingParachainConvertsVia, SignedAccountId32AsNative,
	SovereignSignedViaLocation,
};
use xcm_executor::{
	traits::{IsConcrete, NativeAsset},
	Config, XcmExecutor,
};
use frame_system::limits::{BlockLength, BlockWeights};

// A few exports that help ease life for downstream crates.
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
pub use pallet_timestamp::Call as TimestampCall;
pub use pallet_balances::Call as BalancesCall;
pub use sp_runtime::{Permill, Perbill};
pub use frame_support::{
	construct_runtime, parameter_types, StorageValue,
	traits::{KeyOwnerProofSystem, Randomness},
	weights::{
		Weight, IdentityFee, DispatchClass,
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight, WEIGHT_PER_SECOND},
	},
};

/// myself use
pub use node_primitives::{currency::{LT, DOT}, BlockNumber, Signature, AccountId, AccountIndex, Balance, Index, Hash, Amount, DigestItem, CurrencyId};
pub use node_constants::{time::{SLOT_DURATION, MINUTES, HOURS, DAYS, PRIMARY_PROBABILITY}, currency::{CENTS, MILLICENTS, DOLLARS, deposit}};
use pallet_listen as listen;
use pallet_listen_vesting;
use orml_traits::*;
use orml_tokens;
use pallet_multisig;
use frame_system::{EnsureRoot, EnsureOneOf};
use orml_xtokens;
use sp_runtime::traits::{Convert, Zero};
use cumulus_primitives_core::relay_chain::Balance as RelayChainBalance;
use cumulus_pallet_xcm_handler;
use orml_xcm_support::{
	CurrencyIdConverter, IsConcreteWithGeneralKey, MultiCurrencyAdapter, XcmHandler as XcmHandlerT,
	NativePalletAssetOr,
	// XcmHandler as XcmHandlerT,
};
use pallet_transfer;
use orml_unknown_tokens;
use pallet_nicks;
use pallet_room_collective;
use sp_core::{
	u32_trait::{_1, _2, _3, _4, _5},
};
use orml_currencies::{self, BasicCurrencyAdapter};
use sp_std::collections::btree_set::BTreeSet;
use pallet_room_treasury;


/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
	use super::*;

	pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

	/// Opaque block type.
	pub type Block = generic::Block<Header, UncheckedExtrinsic>;

	pub type SessionHandlers = ();

	impl_opaque_keys! {
		pub struct SessionKeys {}
	}
}

pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("listen-parachain"),
	impl_name: create_runtime_str!("listen-parachain"),
	authoring_version: 1,
	spec_version: 100,
	impl_version: 1,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 1,
};

// 1 in 4 blocks (on average, not counting collisions) will be primary babe blocks.
#[derive(codec::Encode, codec::Decode)]
pub enum XCMPMessage<XAccountId, XBalance> {
	/// Transfer tokens to the given account from the Parachain account.
	TransferToken(XAccountId, XBalance),
}

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion {
		runtime_version: VERSION,
		can_author_with: Default::default(),
	}
}

/// We assume that ~10% of the block weight is consumed by `on_initalize` handlers.
/// This is used to limit the maximal weight of a single extrinsic.
const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(10);
/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used
/// by  Operational  extrinsics.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
/// We allow for 2 seconds of compute with a 6 second average block time.
const MAXIMUM_BLOCK_WEIGHT: Weight = 2 * WEIGHT_PER_SECOND;

parameter_types! {
	pub const BlockHashCount: BlockNumber = 250;
	pub const Version: RuntimeVersion = VERSION;
	pub RuntimeBlockLength: BlockLength =
		BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub RuntimeBlockWeights: BlockWeights = BlockWeights::builder()
		.base_block(BlockExecutionWeight::get())
		.for_class(DispatchClass::all(), |weights| {
			weights.base_extrinsic = ExtrinsicBaseWeight::get();
		})
		.for_class(DispatchClass::Normal, |weights| {
			weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
		})
		.for_class(DispatchClass::Operational, |weights| {
			weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
			// Operational transactions have some extra reserved space, so that they
			// are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
			weights.reserved = Some(
				MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT
			);
		})
		.avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
		.build_or_panic();
	pub const SS58Prefix: u8 = 42;
}

// Configure FRAME pallets to include in runtime.

parameter_types! {
	// One storage item; key size is 32; value is size 4+4+16+32 bytes = 56 bytes.
	pub const DepositBase: Balance = deposit(1, 88);
	// Additional storage item size of 32 bytes.
	pub const DepositFactor: Balance = deposit(0, 32);
	pub const MaxSignatories: u16 = 100;
}

impl pallet_multisig::Config for Runtime {
	type Event = Event;
	type Call = Call;
	type Currency = Balances;
	type DepositBase = DepositBase;
	type DepositFactor = DepositFactor;
	type MaxSignatories = MaxSignatories;
	type WeightInfo = pallet_multisig::weights::SubstrateWeight<Runtime>;
}

impl orml_unknown_tokens::Config for Runtime {
	type Event = Event;
}

pub struct NativeToRelay;
impl Convert<Balance, RelayChainBalance> for NativeToRelay {
	fn convert(val: u128) -> Balance {
		// native is 14
		// relay is 12
		val / 100
	}
}

pub struct AccountId32Convert;
impl Convert<AccountId, [u8; 32]> for AccountId32Convert {
	fn convert(account_id: AccountId) -> [u8; 32] {
		account_id.into()
	}
}

parameter_types! {
	pub const PolkadotNetworkId: NetworkId = NetworkId::Polkadot;
}

pub struct HandleXcm;
impl XcmHandlerT<AccountId> for HandleXcm {
	fn execute_xcm(origin: AccountId, xcm: Xcm) -> DispatchResult {
		XcmHandler::execute_xcm(origin, xcm)
	}
}

impl orml_xtokens::Config for Runtime {
	type Event = Event;
	type Balance = Balance;
	type ToRelayChainBalance = NativeToRelay;
	type AccountId32Convert = AccountId32Convert;

	//TODO: change network id if kusama
	type RelayChainNetworkId = PolkadotNetworkId;
	type ParaId = ParachainInfo;
	 type XcmHandler = HandleXcm;
}

impl frame_system::Config for Runtime {
	/// The basic call filter to use in dispatchable.
	type BaseCallFilter = ();
	/// Block & extrinsics weights: base values and limits.
	type BlockWeights = RuntimeBlockWeights;
	/// The maximum length of a block (in bytes).
	type BlockLength = RuntimeBlockLength;
	/// The identifier used to distinguish between accounts.
	type AccountId = AccountId;
	/// The aggregated dispatch type that is available for extrinsics.
	type Call = Call;
	/// The lookup mechanism to get account ID from whatever is passed in dispatchers.
	type Lookup = AccountIdLookup<AccountId, ()>;
	/// The index type for storing how many extrinsics an account has signed.
	type Index = Index;
	/// The index type for blocks.
	type BlockNumber = BlockNumber;
	/// The type for hashing blocks and tries.
	type Hash = Hash;
	/// The hashing algorithm used.
	type Hashing = BlakeTwo256;
	/// The header type.
	type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// The ubiquitous event type.
	type Event = Event;
	/// The ubiquitous origin type.
	type Origin = Origin;
	/// Maximum number of block number to block hash mappings to keep (oldest pruned first).
	type BlockHashCount = BlockHashCount;
	/// The weight of database operations that the runtime can invoke.
	type DbWeight = RocksDbWeight;
	/// Version of the runtime.
	type Version = Version;
	/// Converts a module to the index of the module in `construct_runtime!`.
	///
	/// This type is being generated by `construct_runtime!`.
	type PalletInfo = PalletInfo;
	/// What to do if a new account is created.
	type OnNewAccount = ();
	/// What to do if an account is fully reaped from the system.
	type OnKilledAccount = ();
	/// The data to be stored in an account.
	type AccountData = pallet_balances::AccountData<Balance>;
	/// Weight information for the extrinsics of this pallet.
	type SystemWeightInfo = ();
	/// This is used as an identifier of the chain. 42 is the generic substrate prefix.
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
}

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = LT;
}

impl orml_currencies::Config for Runtime {
	type Event = Event;
	type MultiCurrency = Tokens;

	type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;

	type GetNativeCurrencyId = GetNativeCurrencyId;

	type WeightInfo = ();

}

parameter_types! {
	pub const RoomMotionDuration: BlockNumber = 5 * DAYS;
	pub const RoomMaxProposals: u32 = 100;
	pub const RoomMaxMembers: u32 = 100;
}

type RoomCollective = pallet_room_collective::Instance1;
impl pallet_room_collective::Config<RoomCollective> for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
	type MotionDuration = RoomMotionDuration;
	type MaxProposals = RoomMaxProposals;
	type MaxMembers = RoomMaxMembers;
	type DefaultVote = pallet_room_collective::PrimeDefaultVote;
	type WeightInfo = pallet_room_collective::weights::SubstrateWeight<Runtime>;
	type ListenHandler = Listen;
}

parameter_types! {
	pub const MinLength: usize = 3;
	pub const MaxLength: usize = 10;
	pub const ReservationFee: Balance = 1*DOLLARS;
}

impl pallet_nicks::Config for Runtime {
	type Event = Event;
	type NicksCurrency = Balances;
	type ReservationFee = ReservationFee;
	type Slashed = ();
	type ForceOrigin = EnsureRoot<AccountId>;
	type MinLength = MinLength;
	type MaxLength = MaxLength;

}


impl pallet_transfer::Config for Runtime {
	type Event = Event;
	type AirDropAmount = AirDropAmount;
}

parameter_types! {
	pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		Zero::zero()
	};
}

impl orml_tokens::Config for Runtime {
	type Event = Event;
	type Balance = Balance;
	type Amount = i128;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
}

parameter_types! {
	pub const ExistentialDeposit: u128 = 500;
	pub const MaxLocks: u32 = 50;
}

impl pallet_balances::Config for Runtime {
	type MaxLocks = MaxLocks;
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// The ubiquitous event type.
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
	pub const TransactionByteFee: Balance = 1;
}

impl pallet_transaction_payment::Config for Runtime {
	type OnChargeTransaction = pallet_transaction_payment::CurrencyAdapter<Balances, ()>;
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = IdentityFee<Balance>;
	type FeeMultiplierUpdate = ();
}

parameter_types! {
	pub const ProposalBondMinimum: Balance = 1 * DOLLARS;
	pub const SpendPeriod: BlockNumber = 20 * MINUTES;
	pub const ProposalBond: Permill = Permill::from_percent(10);
}

impl pallet_room_treasury::Config for Runtime {
	type Event = Event;
	type ApproveOrigin = HalfRoomCouncil;
	type RejectOrigin = HalfRoomCouncil;
	type OnSlash = ();
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ProposalBondMinimum;
	type SpendPeriod = SpendPeriod;
	type WeightInfo = ();
}


impl pallet_sudo::Config for Runtime {
	type Event = Event;
	type Call = Call;
}

parameter_types! {
	pub const MinVestedTransfer: Balance = 100 * DOLLARS;
}

impl pallet_listen_vesting::Config for Runtime {
	type Event = Event;
	type Currency = Balances;
	type BlockNumberToBalance = ConvertInto;
	type MinVestedTransfer = MinVestedTransfer;
	type WeightInfo = ();
}

parameter_types! {
	// ???????????? 0.99???token
	pub const AirDropAmount: Balance = 1 * DOLLARS * 99 / 100;
	pub const RedPacketMinAmount: Balance = 1 * DOLLARS;
	pub const VoteExpire: BlockNumber = 10 * MINUTES;
	pub const ProtectTime: BlockNumber = 5 * MINUTES;
	pub const RedPackExpire: BlockNumber = 30 * MINUTES;
	pub const RewardDuration: BlockNumber = 30 * MINUTES;
	pub const DisbandDelayTime: BlockNumber = 30 * MINUTES;
	pub const ManagerProportion: Percent = Percent::from_percent(1);
	pub const RoomProportion: Percent = Percent::from_percent(1);
	pub const TreasuryModuleId: ModuleId = ModuleId(*b"py/trsry");
	pub const CouncilMaxNumber: u32 = 4;

}

type HalfRoomCouncil = pallet_room_collective::EnsureProportionMoreThan<_1, _2, AccountId, RoomCollective>;
type RoomRoot = pallet_room_collective::EnsureRoomRoot<Runtime, AccountId, RoomCollective>;
type RoomRootOrHalfRoomCouncil = EnsureOneOf<
	AccountId,
	RoomRoot,
	HalfRoomCouncil
>;
type SomeCouncil = pallet_room_collective::EnsureMembers<_2, AccountId, RoomCollective>;
type HalfRoomCouncilOrSomeRoomCouncil = EnsureOneOf<
	AccountId,
	HalfRoomCouncil,
	SomeCouncil,
>;

type RoomRootOrHalfRoomCouncilOrSomeRoomCouncil = EnsureOneOf<
	AccountId,
	RoomRoot,
	HalfRoomCouncilOrSomeRoomCouncil,
>;



impl listen::Config for Runtime{
	type Event = Event;
	type Create = ();
	type NativeCurrency = Balances;

	type MultiCurrency = Tokens;
	type ProposalRejection = ();
	type VoteExpire = VoteExpire;
	type RedPacketMinAmount = RedPacketMinAmount;
	type RedPackExpire = RedPackExpire;
	type RewardDuration = RewardDuration;
	type ManagerProportion = ManagerProportion;
	type RoomProportion = RoomProportion;
	type ModuleId = TreasuryModuleId;
	type AirDropAmount = AirDropAmount;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type ProtectTime = ProtectTime;
	type CouncilMaxNumber = CouncilMaxNumber;
	type CollectiveHandler = RoomCommittee;

	type RoomRootOrigin = RoomRoot;
	type RoomRootOrHalfCouncilOrigin = RoomRootOrHalfRoomCouncil;
	type RoomRootOrHalfRoomCouncilOrSomeRoomCouncilOrigin = RoomRootOrHalfRoomCouncilOrSomeRoomCouncil;
	type HalfRoomCouncilOrigin = HalfRoomCouncil;
	type DisbandDelayTime = DisbandDelayTime;
	type RoomTreasuryHandler = RoomTreasury;

}


impl cumulus_pallet_parachain_system::Config for Runtime {
	type Event = Event;
	type OnValidationData = ();
	type SelfParaId = parachain_info::Module<Runtime>;
	type DownwardMessageHandlers = ();
	// type HrmpMessageHandlers = ();
	type XcmpMessageHandlers = XcmHandler;
}

impl parachain_info::Config for Runtime {}

parameter_types! {
	pub const RococoLocation: MultiLocation = MultiLocation::X1(Junction::Parent);
	pub const RococoNetwork: NetworkId = NetworkId::Polkadot;
	pub RelayChainOrigin: Origin = cumulus_pallet_xcm_handler::Origin::Relay.into();
	pub Ancestry: MultiLocation = Junction::Parachain {
		id: ParachainInfo::parachain_id().into()
	}.into();

	pub const RelayChainCurrencyId: CurrencyId = DOT;
}

type LocationConverter = (
	ParentIsDefault<AccountId>,
	SiblingParachainConvertsVia<Sibling, AccountId>,
	AccountId32Aliases<RococoNetwork, AccountId>,
);


pub struct RelayToNative;
impl Convert<RelayChainBalance, Balance> for RelayToNative {
	fn convert(val: u128) -> Balance {
		// native is 14
		// relay is 12
		val * 100
	}
}

pub type LocalAssetTransactor = MultiCurrencyAdapter<
	Currencies,
	UnknownTokens,
	IsConcreteWithGeneralKey<CurrencyId, RelayToNative>,
	LocationConverter,
	AccountId,
	CurrencyIdConverter<CurrencyId, RelayChainCurrencyId>,
	CurrencyId,
>;



type LocalOriginConverter = (
	SovereignSignedViaLocation<LocationConverter, Origin>,
	RelayChainAsNative<RelayChainOrigin, Origin>,
	SiblingParachainAsNative<cumulus_pallet_xcm_handler::Origin, Origin>,
	SignedAccountId32AsNative<RococoNetwork, Origin>,
);

parameter_types! {
	pub NativeOrmlTokens: BTreeSet<(Vec<u8>, MultiLocation)> = {
		let mut t = BTreeSet::new();
		// TODO ??????????????????????????????????????? ?????????????????????
		// Acala
		t.insert(("ACA".into(), (Junction::Parent, Junction::Parachain { id: 5000 }).into()));
		// phala
		t.insert(("PHA".into(), (Junction::Parent, Junction::Parachain { id: 5000 }).into()));
		t
	};
}


pub struct XcmConfig;
impl Config for XcmConfig {
	type Call = Call;
	type XcmSender = XcmHandler;
	// How to withdraw and deposit an asset.
	type AssetTransactor = LocalAssetTransactor;
	type OriginConverter = LocalOriginConverter;
	type IsReserve = NativePalletAssetOr<NativeOrmlTokens>;
	type IsTeleporter = ();
	type LocationInverter = LocationInverter<Ancestry>;
}

impl cumulus_pallet_xcm_handler::Config for Runtime {
	type Event = Event;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type UpwardMessageSender = ParachainSystem;
	type XcmpMessageSender = ParachainSystem;
	type SendXcmOrigin = EnsureRoot<AccountId>;
	type AccountIdConverter = LocationConverter;
}


// Create the runtime by composing the FRAME pallets that were previously configured.
construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = opaque::Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Sudo: pallet_sudo::{Pallet, Call, Storage, Config<T>, Event<T>},
		RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Pallet, Call, Storage},
		ParachainSystem: cumulus_pallet_parachain_system::{Pallet, Call, Storage, Inherent, Event},
		TransactionPayment: pallet_transaction_payment::{Pallet, Storage},
		ParachainInfo: parachain_info::{Pallet, Storage, Config},
		XcmHandler: cumulus_pallet_xcm_handler::{Pallet, Event<T>, Origin},
		Multisig: pallet_multisig::{Pallet, Call, Storage, Event<T>},
		Tokens: orml_tokens::{Pallet, Config<T>, Storage, Event<T>},
		ListenVesting: pallet_listen_vesting::{Pallet, Call, Storage, Event<T>, Config<T>},
		Listen: listen::{Pallet, Storage, Call, Event<T>},
		XTokens: orml_xtokens::{Pallet, Storage, Call, Event<T>},
		Currencies: orml_currencies::{Pallet, Event<T>},
		Transfer: pallet_transfer::{Pallet, Call, Event<T>},
		UnknownTokens: orml_unknown_tokens::{Pallet, Storage, Event},
		Nicks: pallet_nicks::{Pallet, Storage, Call, Event<T>},
		RoomCommittee: pallet_room_collective::<Instance1>::{Pallet, Call, Storage, Origin<T>, Event<T>},
		RoomTreasury: pallet_room_treasury::{Pallet, Storage, Call, Event<T>},

	}
);

/// The address format for describing accounts.
pub type Address = sp_runtime::MultiAddress<AccountId, ()>;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckEra<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	pallet_transaction_payment::ChargeTransactionPayment<Runtime>
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<Address, Call, Signature, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, Call, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllModules,
>;

impl_runtime_apis! {
	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block)
		}

		fn initialize_block(header: &<Block as BlockT>::Header) {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			Runtime::metadata().into()
		}
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: Block,
			data: sp_inherents::InherentData,
		) -> sp_inherents::CheckInherentsResult {
			data.check_extrinsics(&block)
		}

		fn random_seed() -> <Block as BlockT>::Hash {
			RandomnessCollectiveFlip::random_seed().0
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			opaque::SessionKeys::generate(seed)
		}

		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
			opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
		fn account_nonce(account: AccountId) -> Index {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
		fn query_info(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
		fn query_fee_details(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt, len)
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
			use frame_benchmarking::{Benchmarking, BenchmarkBatch, add_benchmark, TrackedStorageKey};

			use frame_system_benchmarking::Module as SystemBench;
			impl frame_system_benchmarking::Config for Runtime {}

			let whitelist: Vec<TrackedStorageKey> = vec![
				// Block Number
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec().into(),
				// Total Issuance
				hex_literal::hex!("c2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80").to_vec().into(),
				// Execution Phase
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a").to_vec().into(),
				// Event Count
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850").to_vec().into(),
				// System Events
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7").to_vec().into(),
			];

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);

			add_benchmark!(params, batches, frame_system, SystemBench::<Runtime>);
			add_benchmark!(params, batches, pallet_balances, Balances);
			add_benchmark!(params, batches, pallet_timestamp, Timestamp);

			if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
			Ok(batches)
		}
	}
}

cumulus_pallet_parachain_system::register_validate_block!(Runtime, Executive);
