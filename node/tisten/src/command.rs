use crate::{
	chain_spec,
	cli::{Cli, Subcommand},
	service,
};
use listen_runtime::Block;
use sc_cli::{ChainSpec, RuntimeVersion, SubstrateCli};
use sc_service::PartialComponents;

impl SubstrateCli for Cli {
	fn impl_name() -> String {
		"Listen Node".into()
	}

	fn impl_version() -> String {
		env!("SUBSTRATE_CLI_IMPL_VERSION").into()
	}

	fn description() -> String {
		"Listen Node\n\nThe command-line arguments provided first will be \
		passed to the parachain node, while the arguments provided after -- will be passed \
		to the relay chain node.\n\n\
		parachain-collator <parachain-args> -- <relay-chain-args>"
			.into()
	}

	fn author() -> String {
		env!("CARGO_PKG_AUTHORS").into()
	}

	fn support_url() -> String {
		"https://github.com/listenofficial/listen-parachain/issues/new".into()
	}

	fn copyright_start_year() -> i32 {
		2020
	}

	fn load_spec(&self, id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
		Ok(match id {
			"dev" => Box::new(chain_spec::development_config()),
			"test" => Box::new(chain_spec::testnet_config()?),
			"local" => Box::new(chain_spec::local_testnet_config()),
			"staging" => Box::new(chain_spec::staging_config()),
			"" | "main" => Box::new(chain_spec::mainnet_config()?),
			path => Box::new(chain_spec::ChainSpec::from_json_file(std::path::PathBuf::from(path))?),
		})
	}

	fn native_runtime_version(_: &Box<dyn ChainSpec>) -> &'static RuntimeVersion {
		&listen_runtime::VERSION
	}
}

/// Parse and run command line arguments
pub fn run() -> sc_cli::Result<()> {
	let cli = Cli::from_args();

	match &cli.subcommand {
		Some(Subcommand::Key(cmd)) => cmd.run(&cli),
		Some(Subcommand::BuildSpec(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
		},
		Some(Subcommand::CheckBlock(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, import_queue, .. } =
					service::new_partial(&config)?;
				Ok((cmd.run(client, import_queue), task_manager))
			})
		},
		Some(Subcommand::ExportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, .. } = service::new_partial(&config)?;
				Ok((cmd.run(client, config.database), task_manager))
			})
		},
		Some(Subcommand::ExportState(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, .. } = service::new_partial(&config)?;
				Ok((cmd.run(client, config.chain_spec), task_manager))
			})
		},
		Some(Subcommand::ImportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, import_queue, .. } =
					service::new_partial(&config)?;
				Ok((cmd.run(client, import_queue), task_manager))
			})
		},
		Some(Subcommand::PurgeChain(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.database))
		},
		Some(Subcommand::Revert(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, backend, .. } =
					service::new_partial(&config)?;
				Ok((cmd.run(client, backend), task_manager))
			})
		},
		Some(Subcommand::Benchmark(cmd)) =>
			if cfg!(feature = "runtime-benchmarks") {
				let runner = cli.create_runner(cmd)?;

				runner.sync_run(|config| cmd.run::<Block, service::ExecutorDispatch>(config))
			} else {
				Err("Benchmarking wasn't enabled when building the node. You can enable it with \
				     `--features runtime-benchmarks`."
					.into())
			},
		None => {
			let runner = cli.create_runner(&cli.run)?;
			runner.run_node_until_exit(|config| async move {
				service::new_full(config).map_err(sc_cli::Error::Service)
			})
		},
	}
}




// fn load_spec(id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
// 	Ok(match id {
// 		"dev" => Box::new(chain_spec::development_config()),
// 		"test" => Box::new(chain_spec::testnet_config()?),
// 		"local" => Box::new(chain_spec::local_testnet_config()),
// 		"staging" => Box::new(chain_spec::staging_config()),
// 		"" | "main" => Box::new(chain_spec::mainnet_config()?),
// 		path => Box::new(chain_spec::ChainSpec::from_json_file(std::path::PathBuf::from(path))?),
// 	})
// }
//
// impl SubstrateCli for Cli {
// 	fn impl_name() -> String {
// 		"Listen Node".into()
// 	}
//
// 	fn impl_version() -> String {
// 		env!("SUBSTRATE_CLI_IMPL_VERSION").into()
// 	}
//
// 	fn description() -> String {
// 		"Listen Node\n\nThe command-line arguments provided first will be \
// 		passed to the parachain node, while the arguments provided after -- will be passed \
// 		to the relay chain node.\n\n\
// 		parachain-collator <parachain-args> -- <relay-chain-args>"
// 			.into()
// 	}
//
// 	fn author() -> String {
// 		env!("CARGO_PKG_AUTHORS").into()
// 	}
//
// 	fn support_url() -> String {
// 		"https://github.com/listenofficial/listen-parachain/issues/new".into()
// 	}
//
// 	fn copyright_start_year() -> i32 {
// 		2020
// 	}
//
// 	fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
// 		load_spec(id)
// 	}
//
// 	fn native_runtime_version(_: &Box<dyn ChainSpec>) -> &'static RuntimeVersion {
// 		&listen_runtime::VERSION
// 	}
// }
