use super::*;
use pallet_currencies::currencies_trait::AssetIdMapping;

pub fn ksm_per_second() -> u128 {
	let base_weight = Balance::from(ExtrinsicBaseWeight::get());
	let base_tx_fee = UNIT / 1000;
	let base_tx_per_second = (WEIGHT_PER_SECOND as u128) / base_weight;
	let fee_per_second = base_tx_per_second * base_tx_fee;
	fee_per_second / 100
}

// fixme
parameter_types! {
	pub KsmPerSecond: (AssetId, u128) = (MultiLocation::parent().into(), ksm_per_second());

	pub LTPerSecond: (AssetId, u128) = (MultiLocation::new(0,
		X1(GeneralKey(native::lt::TOKEN_SYMBOL.to_vec()))).into(), ksm_per_second() * 100);
	pub LIKEPerSecond: (AssetId, u128) = (MultiLocation::new(0,
		X1(GeneralKey(native::like::TOKEN_SYMBOL.to_vec()))).into(), ksm_per_second() * 100);

	pub KICOPerSecond: (AssetId, u128) = (MultiLocation::new(
				1,
				X2(Parachain(kico::PARA_ID.into()), GeneralKey(kico::kico::TOKEN_SYMBOL.to_vec()))
			).into(), ksm_per_second() * 100);
	pub KTPerSecond: (AssetId, u128) = (MultiLocation::new(
				1,
				X2(Parachain(kisten::PARA_ID.into()), GeneralKey(kisten::kt::TOKEN_SYMBOL.to_vec()))
			).into(), ksm_per_second() * 100);

	pub KUSDPerSecond: (AssetId, u128) = (MultiLocation::new(
				1,
				X2(Parachain(karura::PARA_ID.into()), GeneralKey(karura::kusd::KEY.to_vec()))
			).into(), ksm_per_second() * 100);
	pub AUSDPerSecond: (AssetId, u128) = (MultiLocation::new(
				1,
				X2(Parachain(karura::PARA_ID.into()), GeneralKey(karura::ausd::KEY.to_vec()))
			).into(), ksm_per_second() * 100);
	pub KARPerSecond: (AssetId, u128) = (MultiLocation::new(
				1,
				X2(Parachain(karura::PARA_ID.into()), GeneralKey(karura::kar::KEY.to_vec()))
			).into(), ksm_per_second() * 100);
	pub LKSMPerSecond: (AssetId, u128) = (MultiLocation::new(
				1,
				X2(Parachain(karura::PARA_ID.into()), GeneralKey(karura::lksm::KEY.to_vec()))
			).into(), ksm_per_second() * 100);

	pub BaseRate: u128 = ksm_per_second();
}

pub type Trader = (
	FixedRateOfFungible<KsmPerSecond, ToTreasury>,
	FixedRateOfFungible<LTPerSecond, ToTreasury>,
	FixedRateOfFungible<LIKEPerSecond, ToTreasury>,
	FixedRateOfFungible<KICOPerSecond, ToTreasury>,
	FixedRateOfFungible<KTPerSecond, ToTreasury>,
	FixedRateOfFungible<KUSDPerSecond, ToTreasury>,
	FixedRateOfFungible<AUSDPerSecond, ToTreasury>,
	FixedRateOfFungible<KARPerSecond, ToTreasury>,
	FixedRateOfFungible<LKSMPerSecond, ToTreasury>,
	FixedRateOfAsset<BaseRate, ToTreasury, pallet_currencies::AssetIdMaps<Runtime>>,
);

fn native_currency_location(id: CurrencyId) -> Option<MultiLocation> {
	let token_symbol = match id {
		native::lt::ASSET_ID => native::lt::TOKEN_SYMBOL,
		native::like::ASSET_ID => native::like::TOKEN_SYMBOL,
		_ => return None,
	};
	Some(MultiLocation::new(
		1,
		X2(Parachain(ParachainInfo::parachain_id().into()), GeneralKey(token_symbol.to_vec())),
	))
}

pub struct CurrencyIdConvert;

impl Convert<CurrencyId, Option<MultiLocation>> for CurrencyIdConvert {
	fn convert(id: CurrencyId) -> Option<MultiLocation> {
		if let Some(i) = pallet_currencies::AssetIdMaps::<Runtime>::get_multi_location(id) {
			return Some(i)
		}
		match id {
			kusama::ksm::ASSET_ID => Some(MultiLocation::parent()),

			native::lt::ASSET_ID | native::like::ASSET_ID => native_currency_location(id),

			kico::kico::ASSET_ID => Some(MultiLocation::new(
				1,
				X2(Parachain(kico::PARA_ID.into()), GeneralKey(kico::kico::TOKEN_SYMBOL.to_vec())),
			)),
			kisten::kt::ASSET_ID => Some(MultiLocation::new(
				1,
				X2(
					Parachain(kisten::PARA_ID.into()),
					GeneralKey(kisten::kt::TOKEN_SYMBOL.to_vec()),
				),
			)),

			karura::ausd::ASSET_ID => Some(MultiLocation::new(
				1,
				X2(Parachain(karura::PARA_ID.into()), GeneralKey(karura::ausd::KEY.to_vec())),
			)),
			karura::kusd::ASSET_ID => Some(MultiLocation::new(
				1,
				X2(Parachain(karura::PARA_ID.into()), GeneralKey(karura::kusd::KEY.to_vec())),
			)),
			karura::kar::ASSET_ID => Some(MultiLocation::new(
				1,
				X2(Parachain(karura::PARA_ID.into()), GeneralKey(karura::kar::KEY.to_vec())),
			)),
			karura::lksm::ASSET_ID => Some(MultiLocation::new(
				1,
				X2(Parachain(karura::PARA_ID.into()), GeneralKey(karura::lksm::KEY.to_vec())),
			)),

			_ => None,
		}
	}
}

impl Convert<MultiLocation, Option<CurrencyId>> for CurrencyIdConvert {
	fn convert(location: MultiLocation) -> Option<CurrencyId> {
		if location == MultiLocation::parent() {
			return Some(kusama::ksm::ASSET_ID.into())
		}

		if let Some(l) =
			pallet_currencies::AssetIdMaps::<Runtime>::get_currency_id(location.clone())
		{
			return Some(l)
		}

		match location {
			MultiLocation { parents: 1, interior: X2(Parachain(para_id), GeneralKey(key)) } =>
				match (para_id, &key[..]) {
					(kico::PARA_ID, kico::kico::TOKEN_SYMBOL) => Some(kico::kico::ASSET_ID.into()),
					(kisten::PARA_ID, kisten::kt::TOKEN_SYMBOL) =>
						Some(kisten::kt::ASSET_ID.into()),

					(karura::PARA_ID, karura::ausd::KEY) => Some(karura::ausd::ASSET_ID.into()),
					(karura::PARA_ID, karura::kar::KEY) => Some(karura::kar::ASSET_ID.into()),
					(karura::PARA_ID, karura::kusd::KEY) => Some(karura::kusd::ASSET_ID.into()),
					(karura::PARA_ID, karura::lksm::KEY) => Some(karura::lksm::ASSET_ID.into()),

					(id, key) if id == u32::from(ParachainInfo::parachain_id()) => match key {
						native::lt::TOKEN_SYMBOL => Some(native::lt::ASSET_ID.into()),
						native::like::TOKEN_SYMBOL => Some(native::like::ASSET_ID.into()),
						_ => None,
					},
					_ => None,
				},

			MultiLocation { parents: 0, interior: X1(GeneralKey(key)) } => match &key[..] {
				native::lt::TOKEN_SYMBOL => Some(native::lt::ASSET_ID),
				native::like::TOKEN_SYMBOL => Some(native::like::ASSET_ID),
				_ => None,
			},
			_ => None,
		}
	}
}

impl Convert<MultiAsset, Option<CurrencyId>> for CurrencyIdConvert {
	fn convert(asset: MultiAsset) -> Option<CurrencyId> {
		if let MultiAsset { id: Concrete(location), .. } = asset {
			Self::convert(location)
		} else {
			None
		}
	}
}

parameter_types! {
	pub const RelayLocation: MultiLocation = MultiLocation::parent();
	pub const RelayNetwork: NetworkId = NetworkId::Any;
	pub RelayChainOrigin: Origin = cumulus_pallet_xcm::Origin::Relay.into();
	pub Ancestry: MultiLocation = Parachain(ParachainInfo::parachain_id().into()).into();
}

/// Type for specifying how a `MultiLocation` can be converted into an `AccountId`. This is used
/// when determining ownership of accounts for asset transacting and when attempting to use XCM
/// `Transact` in order to determine the dispatch Origin.
pub type LocationToAccountId = (
	// The parent (Relay-chain) origin converts to the default `AccountId`.
	ParentIsPreset<AccountId>,
	// Sibling parachain origins convert to AccountId via the `ParaId::into`.
	SiblingParachainConvertsVia<Sibling, AccountId>,
	// Straight up local `AccountId32` origins just alias directly to `AccountId`.
	AccountId32Aliases<RelayNetwork, AccountId>,
);

parameter_types! {
	pub ListenTreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
}

pub type LocalAssetTransactor = MultiCurrencyAdapter<
	Currencies,
	UnknownTokens,
	IsNativeConcrete<CurrencyId, CurrencyIdConvert>,
	AccountId,
	LocationToAccountId,
	CurrencyId,
	CurrencyIdConvert,
	DepositToAlternative<ListenTreasuryAccount, Currencies, CurrencyId, AccountId, Balance>,
>;

/// This is the type we use to convert an (incoming) XCM origin into a local `Origin` instance,
/// ready for dispatching a transaction with Xcm's `Transact`. There is an `OriginKind` which can
/// biases the kind of local `Origin` it will become.
pub type XcmOriginToTransactDispatchOrigin = (
	// Sovereign account converter; this attempts to derive an `AccountId` from the origin location
	// using `LocationToAccountId` and then turn that into the usual `Signed` origin. Useful for
	// foreign chains who want to have a local sovereign account on this chain which they control.
	SovereignSignedViaLocation<LocationToAccountId, Origin>,
	// Native converter for Relay-chain (Parent) location; will converts to a `Relay` origin when
	// recognised.
	RelayChainAsNative<RelayChainOrigin, Origin>,
	// Native converter for sibling Parachains; will convert to a `SiblingPara` origin when
	// recognised.
	SiblingParachainAsNative<cumulus_pallet_xcm::Origin, Origin>,
	// Superuser converter for the Relay-chain (Parent) location. This will allow it to issue a
	// transaction from the Root origin.
	ParentAsSuperuser<Origin>,
	// Native signed account converter; this just converts an `AccountId32` origin into a normal
	// `Origin::Signed` origin of the same 32-byte value.
	SignedAccountId32AsNative<RelayNetwork, Origin>,
	// Xcm origins can be represented natively under the Xcm pallet's Xcm origin.
	XcmPassthrough<Origin>,
);

parameter_types! {
	// One XCM operation is 1_000_000_000 weight - almost certainly a conservative estimate.
	pub UnitWeightCost: Weight = 200_000_000;
	pub const MaxInstructions: u32 = 100;
}

match_types! {
	pub type ParentOrParentsExecutivePlurality: impl Contains<MultiLocation> = {
		MultiLocation { parents: 1, interior: Here } |
		MultiLocation { parents: 1, interior: X1(Plurality { id: BodyId::Executive, .. }) }
	};
}

pub type Barrier = (
	TakeWeightCredit,
	AllowTopLevelPaidExecutionFrom<Everything>,
	// Expected responses are OK.
	AllowKnownQueryResponses<PolkadotXcm>,
	// Subscriptions for version tracking are OK.
	AllowSubscriptionsFrom<Everything>,
	AllowUnpaidExecutionFrom<ParentOrParentsExecutivePlurality>,
	// ^^^ Parent and its exec plurality get free execution
);

/// No local origins on this chain are allowed to dispatch XCM sends/executions.
/// fixme
pub type LocalOriginToLocation = SignedToAccountId32<Origin, AccountId, RelayNetwork>;

/// The means for routing XCM messages which are not for local execution into the right message
/// queues.
pub type XcmRouter = (
	// Two routers - use UMP to communicate with the relay chain:
	cumulus_primitives_utility::ParentAsUmp<ParachainSystem, PolkadotXcm>,
	// ..and XCMP to communicate with the sibling chains.
	XcmpQueue,
);

parameter_types! {
	pub TreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
}

pub struct ToTreasury;
impl TakeRevenue for ToTreasury {
	fn take_revenue(revenue: MultiAsset) {
		if let MultiAsset { id: Concrete(location), fun: Fungible(amount) } = revenue {
			if let Some(currency_id) = CurrencyIdConvert::convert(location) {
				// ensure KaruraTreasuryAccount have ed for all of the cross-chain asset.
				// Ignore the result.
				let _ = Currencies::deposit(currency_id, &TreasuryAccount::get(), amount);
			}
		}
	}
}

/// fixme
pub struct XcmConfig;
impl Config for XcmConfig {
	type Call = Call;
	type XcmSender = XcmRouter;
	// How to withdraw and deposit an asset.
	type AssetTransactor = LocalAssetTransactor;
	type OriginConverter = XcmOriginToTransactDispatchOrigin;
	type IsReserve = MultiNativeAsset<AbsoluteReserveProvider>;
	type IsTeleporter = (); // Should be enough to allow teleportation of ROC
	type LocationInverter = LocationInverter<Ancestry>;
	type Barrier = Barrier;
	type Weigher = FixedWeightBounds<UnitWeightCost, Call, MaxInstructions>;
	/// fixme
	type Trader = Trader;
	type ResponseHandler = PolkadotXcm;
	type AssetTrap = PolkadotXcm;
	type AssetClaims = PolkadotXcm;
	type SubscriptionService = PolkadotXcm;
}

impl cumulus_pallet_xcmp_queue::Config for Runtime {
	type Event = Event;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type ChannelInfo = ParachainSystem;
	type VersionWrapper = PolkadotXcm;
	type ExecuteOverweightOrigin = EnsureRoot<AccountId>;
	type ControllerOrigin = EnsureRoot<AccountId>;
	type ControllerOriginConverter = XcmOriginToTransactDispatchOrigin;
	type WeightInfo = ();
}

/// XcmRouter LocalOriginToLocation XcmConfig
impl pallet_xcm::Config for Runtime {
	type Event = Event;
	type SendXcmOrigin = EnsureXcmOrigin<Origin, LocalOriginToLocation>;
	type XcmRouter = XcmRouter;
	type ExecuteXcmOrigin = EnsureXcmOrigin<Origin, LocalOriginToLocation>;
	type XcmExecuteFilter = Nothing;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type XcmTeleportFilter = Nothing;
	type XcmReserveTransferFilter = Everything;
	type Weigher = FixedWeightBounds<UnitWeightCost, Call, MaxInstructions>;
	type LocationInverter = LocationInverter<Ancestry>;
	type Origin = Origin;
	type Call = Call;

	const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
	type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
}

impl cumulus_pallet_xcm::Config for Runtime {
	type Event = Event;
	type XcmExecutor = XcmExecutor<XcmConfig>;
}

impl cumulus_pallet_dmp_queue::Config for Runtime {
	type Event = Event;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type ExecuteOverweightOrigin = EnsureRoot<AccountId>;
}

impl orml_unknown_tokens::Config for Runtime {
	type Event = Event;
}

impl orml_xcm::Config for Runtime {
	type Event = Event;
	type SovereignOrigin = EnsureRootOrThreeFourthsCouncil;
}

pub struct AccountIdToMultiLocation;
impl Convert<AccountId, MultiLocation> for AccountIdToMultiLocation {
	fn convert(account: AccountId) -> MultiLocation {
		X1(AccountId32 { network: NetworkId::Any, id: account.into() }).into()
	}
}

parameter_types! {
	pub const BaseXcmWeight: Weight = 100_000_000;
	pub SelfLocation: MultiLocation = MultiLocation::new(1, X1(Parachain(ParachainInfo::parachain_id().into())));
	pub const MaxAssetsForTransfer: usize = 2;
}

parameter_type_with_key! {
	pub ParachainMinFee: |location: MultiLocation| -> Option<u128> {
		#[allow(clippy::match_ref_pats)] // false positive
		match (location.parents, location.first_interior()) {
			(1, Some(Parachain(statemine::PARA_ID))) => Some(4_000_000_000),
			_ => Some(u128::MAX),
		}
	};
}

impl orml_xtokens::Config for Runtime {
	type Event = Event;
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type CurrencyIdConvert = CurrencyIdConvert;
	type AccountIdToMultiLocation = AccountIdToMultiLocation;
	type SelfLocation = SelfLocation;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type Weigher = FixedWeightBounds<UnitWeightCost, Call, MaxInstructions>;
	type BaseXcmWeight = BaseXcmWeight;
	type LocationInverter = LocationInverter<Ancestry>;
	type MaxAssetsForTransfer = MaxAssetsForTransfer;
	type MinXcmFee = ParachainMinFee;
	type MultiLocationsFilter = Everything;
	type ReserveProvider = AbsoluteReserveProvider;
}

parameter_types! {
	pub const ReservedXcmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT / 4;
	pub const ReservedDmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT / 4;
}

impl cumulus_pallet_parachain_system::Config for Runtime {
	type Event = Event;
	type OnSystemEvent = ();
	type SelfParaId = parachain_info::Pallet<Runtime>;
	type DmpMessageHandler = DmpQueue;
	type ReservedDmpWeight = ReservedDmpWeight;
	type OutboundXcmpMessageSource = XcmpQueue;
	type XcmpMessageHandler = XcmpQueue;
	type ReservedXcmpWeight = ReservedXcmpWeight;
}
