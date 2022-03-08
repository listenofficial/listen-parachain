use cumulus_primitives_core::ParaId;
use listen_primitives::{constants::currency::UNIT, Balance};
use listen_runtime::{
	AccountId, AuraId, CouncilConfig, CurrenciesConfig, ElectionsConfig, ListenConfig, Signature,
	SudoConfig, TechnicalCommitteeConfig, VestingConfig, EXISTENTIAL_DEPOSIT,
};
use pallet_currencies::{ListenAssetInfo, ListenAssetMetadata};
use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use sc_service::{ChainType, Properties};
use sc_telemetry::TelemetryEndpoints;
use serde::{Deserialize, Serialize};
use sp_core::{crypto::Ss58Codec, sr25519, Pair, Public};
use sp_runtime::{
	traits::{IdentifyAccount, Verify},
	AccountId32,
};

/// Specialized `ChainSpec` for the normal parachain runtime.
pub type ChainSpec = sc_service::GenericChainSpec<listen_runtime::GenesisConfig, Extensions>;

/// Helper function to generate a crypto pair from seed
pub fn get_pair_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

pub fn testnet_config() -> Result<ChainSpec, String> {
	ChainSpec::from_json_bytes(&include_bytes!("../res/local-testnet.json")[..])
}

const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";
const PARA_ID: u32 = 1002;

/// The extensions for the [`ChainSpec`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ChainSpecGroup, ChainSpecExtension)]
#[serde(deny_unknown_fields)]
pub struct Extensions {
	/// The relay chain of the Parachain.
	pub relay_chain: String,
	/// The id of the Parachain.
	pub para_id: u32,
}

fn get_telemetry_endpoints() -> Option<TelemetryEndpoints> {
	Some(
		TelemetryEndpoints::new(vec![(STAGING_TELEMETRY_URL.to_string(), 0)])
			.expect("Staging telemetry url is valid; qed"),
	)
}

impl Extensions {
	/// Try to get the extension from the given `ChainSpec`.
	pub fn try_get(chain_spec: &dyn sc_service::ChainSpec) -> Option<&Self> {
		sc_chain_spec::get_extension(chain_spec.extensions())
	}
}

type AccountPublic = <Signature as Verify>::Signer;

/// Generate collator keys from seed.
///
/// This function's return type must always match the session keys of the chain in tuple format.
pub fn get_collator_keys_from_seed(seed: &str) -> AuraId {
	get_pair_from_seed::<AuraId>(seed)
}

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_pair_from_seed::<TPublic>(seed)).into_account()
}

/// Generate the session keys from individual elements.
///
/// The input must be a tuple of individual keys (a single arg for now since we have just one key).
pub fn template_session_keys(keys: AuraId) -> listen_runtime::SessionKeys {
	listen_runtime::SessionKeys { aura: keys }
}

pub fn development_config() -> ChainSpec {
	ChainSpec::from_genesis(
		// Name
		"Development",
		// ID
		"dev",
		ChainType::Development,
		move || {
			testnet_genesis(
				// initial collators.
				vec![
					(
						get_account_id_from_seed::<sr25519::Public>("Alice"),
						get_collator_keys_from_seed("Alice"),
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Bob"),
						get_collator_keys_from_seed("Bob"),
					),
				],
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Charlie"),
					get_account_id_from_seed::<sr25519::Public>("Dave"),
					get_account_id_from_seed::<sr25519::Public>("Eve"),
					get_account_id_from_seed::<sr25519::Public>("Ferdie"),
					get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
					get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
					get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
					get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
					get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
					get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
				],
				PARA_ID.into(),
			)
		},
		vec![],
		get_telemetry_endpoints(),
		Some("listen-dev"),
		None,
		Some(get_properties()),
		Extensions {
			relay_chain: "rococo-dev".into(), // You MUST set this to the correct network!
			para_id: PARA_ID.into(),
		},
	)
}

fn get_properties() -> Properties {
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("tokenSymbol".into(), "LT".into());
	properties.insert("tokenDecimals".into(), 12.into());
	properties.insert("ss58Format".into(), 42.into());
	properties
}

fn get_root() -> AccountId {
	AccountId32::from_string("5FQyoSCbcnodfunhcC7ZpwKkad8JSFxLaZ54aoZyb7HXoX3h").unwrap()
}

pub fn local_testnet_config() -> ChainSpec {
	ChainSpec::from_genesis(
		// Name
		"listen testnet",
		// ID
		"listen_testnet",
		ChainType::Local,
		move || {
			testnet_genesis(
				// initial collators.
				vec![
					(
						get_account_id_from_seed::<sr25519::Public>("Alice"),
						get_collator_keys_from_seed("Alice"),
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Bob"),
						get_collator_keys_from_seed("Bob"),
					),
				],
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Charlie"),
					get_account_id_from_seed::<sr25519::Public>("Dave"),
					get_account_id_from_seed::<sr25519::Public>("Eve"),
					get_account_id_from_seed::<sr25519::Public>("Ferdie"),
					get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
					get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
					get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
					get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
					get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
					get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
				],
				PARA_ID.into(),
			)
		},
		// Bootnodes
		vec![],
		// Telemetry
		get_telemetry_endpoints(),
		// Protocol ID
		Some("listen"),
		// Properties
		None,
		Some(get_properties()),
		// Extensions
		Extensions {
			relay_chain: "rococo-local".into(), // You MUST set this to the correct network!
			para_id: PARA_ID.into(),
		},
	)
}

pub const ENDOWMENT: Balance = 10_000_000 * UNIT;
pub const STASH: Balance = 100 * UNIT;

fn testnet_genesis(
	invulnerables: Vec<(AccountId, AuraId)>,
	endowed_accounts: Vec<AccountId>,
	id: ParaId,
) -> listen_runtime::GenesisConfig {
	listen_runtime::GenesisConfig {
		system: listen_runtime::SystemConfig {
			code: listen_runtime::WASM_BINARY
				.expect("WASM binary was not build, please build it!")
				.to_vec(),
		},
		balances: listen_runtime::BalancesConfig {
			balances: endowed_accounts.iter().cloned().map(|k| (k, ENDOWMENT)).collect(),
		},
		parachain_info: listen_runtime::ParachainInfoConfig { parachain_id: id },
		collator_selection: listen_runtime::CollatorSelectionConfig {
			invulnerables: invulnerables.iter().cloned().map(|(acc, _)| acc).collect(),
			candidacy_bond: EXISTENTIAL_DEPOSIT * 16,
			..Default::default()
		},
		elections: ElectionsConfig {
			members: invulnerables.iter().cloned().map(|(a, _)| (a, STASH)).collect(),
			// phantom: Default::default(),
		},
		technical_committee: Default::default(),
		council: Default::default(),
		democracy: Default::default(),
		vesting: Default::default(),
		session: listen_runtime::SessionConfig {
			keys: invulnerables
				.iter()
				.cloned()
				.map(|(acc, aura)| {
					(
						acc.clone(),                 // account id
						acc,                         // validator id
						template_session_keys(aura), // session keys
					)
				})
				.collect(),
		},
		// no need to pass anything to aura, in fact it will panic if we do. Session will take care
		// of this.
		aura: Default::default(),
		aura_ext: Default::default(),
		parachain_system: Default::default(),
		tokens: Default::default(),
		sudo: SudoConfig { key: Some(get_root()) },
		listen: ListenConfig {
			server_id: Some(get_root()),
			multisig_members: vec![
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				get_account_id_from_seed::<sr25519::Public>("Bob"),
				get_account_id_from_seed::<sr25519::Public>("Charlie"),
			],
		},
		currencies: CurrenciesConfig {
			assets: vec![
				(
					0,
					ListenAssetInfo {
						owner: get_root(),
						metadata: Some(ListenAssetMetadata {
							name: "listen".into(),
							symbol: "LT".into(),
							decimals: 12u8,
						}),
					},
					ENDOWMENT,
				),
				(
					101,
					ListenAssetInfo {
						owner: get_root(),
						metadata: Some(ListenAssetMetadata {
							name: "listen point".into(),
							symbol: "LTP".into(),
							decimals: 12u8,
						}),
					},
					ENDOWMENT,
				),
			],
		},
	}
}
