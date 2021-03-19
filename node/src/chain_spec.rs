use cumulus_primitives_core::ParaId;
use parachain_runtime::{AccountId, Signature,
						TokensConfig, ListenVestingConfig,};

use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup, Properties};
use sc_service::ChainType;
use serde::{Deserialize, Serialize};
use sp_core::{sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};
use sc_telemetry::TelemetryEndpoints;

use node_primitives::currency::*;
use hex_literal::hex;

/// Specialized `ChainSpec` for the normal parachain runtime.
pub type ChainSpec = sc_service::GenericChainSpec<parachain_runtime::GenesisConfig, Extensions>;

const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// The extensions for the [`ChainSpec`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ChainSpecGroup, ChainSpecExtension)]
#[serde(deny_unknown_fields)]
pub struct Extensions {
	/// The relay chain of the Parachain.
	pub relay_chain: String,
	/// The id of the Parachain.
	pub para_id: u32,
}

impl Extensions {
	/// Try to get the extension from the given `ChainSpec`.
	pub fn try_get(chain_spec: &dyn sc_service::ChainSpec) -> Option<&Self> {
		sc_chain_spec::get_extension(chain_spec.extensions())
	}
}

type AccountPublic = <Signature as Verify>::Signer;

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

pub fn development_config(id: ParaId) -> ChainSpec {
	ChainSpec::from_genesis(
		// Name
		"Development",
		// ID
		"dev",
		ChainType::Local,
		move || {
			testnet_genesis(
				hex!["a0921eeb61d94111a26536f0218a53055e5c19d970e4f9cf51167437adf86860"].into(),
				vec![
					hex!["a0921eeb61d94111a26536f0218a53055e5c19d970e4f9cf51167437adf86860"].into(),
				],
				id,
			)
		},
		vec![],
		None,
		None,
		None,
		Extensions {
			relay_chain: "rococo-dev".into(),
			para_id: id.into(),
		},
	)
}

pub fn local_testnet_config(id: ParaId) -> ChainSpec {

	let properties = get_properties();
	ChainSpec::from_genesis(
		// Name
		"Listen Local Testnet",
		// ID
		"listen_local_testnet",
		ChainType::Local,
		move || {
			testnet_genesis(
				hex!["a0921eeb61d94111a26536f0218a53055e5c19d970e4f9cf51167437adf86860"].into(),
				vec![
					hex!["a0921eeb61d94111a26536f0218a53055e5c19d970e4f9cf51167437adf86860"].into(),
				],
				id,
			)
		},

		vec![],
		Some(TelemetryEndpoints::new(vec![(STAGING_TELEMETRY_URL.to_string(), 0)])
			.expect("Staging telemetry url is valid; qed")),
		None,
		properties,
		Extensions {
			relay_chain: "rococo-local".into(),
			para_id: id.into(),
		},
	)
}

fn get_properties() -> Option<Properties> {
	let mut p = Properties::new();
	p.insert("tokenSymbol".into(), "LT".into());
	p.insert("tokenDecimals".into(), 14.into());
	Some(p)

}

fn testnet_genesis(
	root_key: AccountId,
	endowed_accounts: Vec<AccountId>,
	id: ParaId,
) -> parachain_runtime::GenesisConfig {
	parachain_runtime::GenesisConfig {
		frame_system: parachain_runtime::SystemConfig {
			code: parachain_runtime::WASM_BINARY
				.expect("WASM binary was not build, please build it!")
				.to_vec(),
			changes_trie_config: Default::default(),
		},

		pallet_balances: parachain_runtime::BalancesConfig {
			balances: endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, 1 << 60))
				.collect(),
		},

		pallet_sudo: parachain_runtime::SudoConfig { key: root_key.clone() },
		parachain_info: parachain_runtime::ParachainInfoConfig { parachain_id: id },

		orml_tokens: TokensConfig {
			endowed_accounts: vec![
				(root_key.clone(), DOT, 1 << 60),
				(root_key.clone(), KSM, 1 << 60),
				(root_key.clone(), BTC, 1 << 60),
				(root_key.clone(), ACA, 1 << 60),
			]

		},

		pallet_listen_vesting: Default::default(),
	}
}
