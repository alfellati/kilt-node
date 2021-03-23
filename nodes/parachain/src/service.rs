// KILT Blockchain – https://botlabs.org
// Copyright (C) 2019-2021 BOTLabs GmbH

// The KILT Blockchain is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The KILT Blockchain is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

// If you feel like getting in touch with us, you can do so at info@botlabs.org

use cumulus_client_consensus_relay_chain::{build_relay_chain_consensus, BuildRelayChainConsensusParams};
use cumulus_client_network::build_block_announce_validator;
use cumulus_client_service::{
	prepare_node_config, start_collator, start_full_node, StartCollatorParams, StartFullNodeParams,
};
use cumulus_primitives_core::ParaId;
use kilt_parachain_runtime::RuntimeApi;
use kilt_primitives::Block;
use polkadot_primitives::v0::CollatorPair;
use sc_executor::native_executor_instance;
pub use sc_executor::NativeExecutor;
use sc_service::{Configuration, PartialComponents, Role, TFullBackend, TFullClient, TaskManager};
use sc_telemetry::TelemetrySpan;
use sp_core::Pair;
use sp_runtime::traits::BlakeTwo256;
use sp_trie::PrefixedMemoryDB;
use std::sync::Arc;

// Native executor instance.
native_executor_instance!(
	pub Executor,
	kilt_parachain_runtime::api::dispatch,
	kilt_parachain_runtime::native_version,
);

type PartialComponentsType = sc_service::PartialComponents<
	TFullClient<Block, RuntimeApi, Executor>,
	TFullBackend<Block>,
	(),
	sp_consensus::import_queue::BasicQueue<Block, PrefixedMemoryDB<BlakeTwo256>>,
	sc_transaction_pool::FullPool<Block, TFullClient<Block, RuntimeApi, Executor>>,
	(),
>;

/// Starts a `ServiceBuilder` for a full service.
///
/// Use this macro if you don't actually need the full service, but just the
/// builder in order to be able to perform chain operations.
pub fn new_partial(config: &Configuration) -> Result<PartialComponentsType, sc_service::Error> {
	let inherent_data_providers = sp_inherents::InherentDataProviders::new();

	let (client, backend, keystore_container, task_manager) =
		sc_service::new_full_parts::<Block, RuntimeApi, Executor>(&config)?;
	let client = Arc::new(client);

	let registry = config.prometheus_registry();

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.role.is_authority().into(),
		config.prometheus_registry(),
		task_manager.spawn_handle(),
		client.clone(),
	);

	let import_queue = cumulus_client_consensus_relay_chain::import_queue(
		client.clone(),
		client.clone(),
		inherent_data_providers.clone(),
		&task_manager.spawn_essential_handle(),
		registry.clone(),
	)?;

	let params = PartialComponents {
		backend,
		client,
		import_queue,
		keystore_container,
		task_manager,
		transaction_pool,
		inherent_data_providers,
		select_chain: (),
		other: (),
	};

	Ok(params)
}

/// Start a node with the given parachain `Configuration` and relay chain
/// `Configuration`.
///
/// This is the actual implementation that is abstract over the executor and the
/// runtime api.
#[sc_tracing::logging::prefix_logs_with("Parachain")]
async fn start_node_impl(
	parachain_config: Configuration,
	collator_key: CollatorPair,
	polkadot_config: Configuration,
	id: ParaId,
	validator: bool,
) -> sc_service::error::Result<(TaskManager, Arc<TFullClient<Block, RuntimeApi, Executor>>)> {
	if matches!(parachain_config.role, Role::Light) {
		return Err("Light client not supported!".into());
	}

	let parachain_config = prepare_node_config(parachain_config);

	let polkadot_full_node = cumulus_client_service::build_polkadot_full_node(polkadot_config, collator_key.public())
		.map_err(|e| match e {
		polkadot_service::Error::Sub(x) => x,
		s => format!("{}", s).into(),
	})?;

	let params = new_partial(&parachain_config)?;
	params
		.inherent_data_providers
		.register_provider(sp_timestamp::InherentDataProvider)
		.unwrap();

	let client = params.client.clone();
	let backend = params.backend.clone();
	let block_announce_validator = build_block_announce_validator(
		polkadot_full_node.client.clone(),
		id,
		Box::new(polkadot_full_node.network.clone()),
		polkadot_full_node.backend.clone(),
	);

	let prometheus_registry = parachain_config.prometheus_registry().cloned();
	let transaction_pool = params.transaction_pool.clone();
	let mut task_manager = params.task_manager;
	let import_queue = params.import_queue;
	let (network, network_status_sinks, system_rpc_tx, start_network) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &parachain_config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue,
			on_demand: None,
			block_announce_validator_builder: Some(Box::new(|_| block_announce_validator)),
		})?;

	let rpc_extensions_builder = {
		let client = client.clone();
		let pool = transaction_pool.clone();
		Box::new(move |deny_unsafe, _| {
			let deps = crate::rpc::FullDeps {
				client: client.clone(),
				pool: pool.clone(),
				deny_unsafe,
			};

			crate::rpc::create_full(deps)
		})
	};

	let telemetry_span = TelemetrySpan::new();
	let _telemetry_span_entered = telemetry_span.enter();

	sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		on_demand: None,
		remote_blockchain: None,
		rpc_extensions_builder,
		client: client.clone(),
		transaction_pool: transaction_pool.clone(),
		task_manager: &mut task_manager,
		config: parachain_config,
		keystore: params.keystore_container.sync_keystore(),
		backend: backend.clone(),
		network: network.clone(),
		network_status_sinks,
		system_rpc_tx,
		telemetry_span: Some(telemetry_span.clone()),
	})?;

	let announce_block = {
		let network = network.clone();
		Arc::new(move |hash, data| network.announce_block(hash, Some(data)))
	};

	if validator {
		let proposer_factory = sc_basic_authorship::ProposerFactory::with_proof_recording(
			task_manager.spawn_handle(),
			client.clone(),
			transaction_pool,
			prometheus_registry.as_ref(),
		);
		let spawner = task_manager.spawn_handle();

		let parachain_consensus = build_relay_chain_consensus(BuildRelayChainConsensusParams {
			para_id: id,
			proposer_factory,
			inherent_data_providers: params.inherent_data_providers,
			block_import: client.clone(),
			relay_chain_client: polkadot_full_node.client.clone(),
			relay_chain_backend: polkadot_full_node.backend.clone(),
		});

		let params = StartCollatorParams {
			para_id: id,
			block_status: client.clone(),
			announce_block,
			client: client.clone(),
			task_manager: &mut task_manager,
			collator_key,
			relay_chain_full_node: polkadot_full_node,
			spawner,
			backend,
			parachain_consensus,
		};

		start_collator(params).await?;
	} else {
		let params = StartFullNodeParams {
			client: client.clone(),
			announce_block,
			task_manager: &mut task_manager,
			para_id: id,
			polkadot_full_node,
		};

		start_full_node(params)?;
	}

	start_network.start_network();

	Ok((task_manager, client))
}

/// Start a normal parachain node.
pub async fn start_node(
	parachain_config: Configuration,
	collator_key: CollatorPair,
	polkadot_config: Configuration,
	id: ParaId,
	validator: bool,
) -> sc_service::error::Result<(TaskManager, Arc<TFullClient<Block, RuntimeApi, Executor>>)> {
	start_node_impl(parachain_config, collator_key, polkadot_config, id, validator).await
}
