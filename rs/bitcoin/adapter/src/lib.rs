#![warn(missing_docs)]

//! The Bitcoin adapter interacts with the Bitcoin P2P network to obtain blocks
//! and publish transactions. Moreover, it interacts with the Bitcoin system
//! component to provide blocks and collect outgoing transactions.

use bitcoin::{network::message::NetworkMessage, BlockHash, BlockHeader};
use std::time::Duration;
use ic_logger::ReplicaLogger;
use ic_metrics::MetricsRegistry;
use parking_lot::RwLock;
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::{
    sync::{mpsc::channel, watch},
    time::interval,
};
/// This module contains the AddressManager struct. The struct stores addresses
/// that will be used to create new connections. It also tracks addresses that
/// are in current use to encourage use from non-utilized addresses.
mod addressbook;
/// This module contains method for managing the local Bitcoin ledger,
/// sending "getheaders", "getdata" messages to Bitcoin peers,
/// processing the "inv", "headers", "block" messages received from Bitcoin peers, and
/// answering the queries of Bitcoin system component.
mod blockchainmanager;
/// This module contains the data structure for storing the current state of the Bitcoin ledger
mod blockchainstate;
/// This module contains command line arguments parser.
pub mod cli;
/// This module contains constants and types that are shared by many modules.
mod common;
/// This module contains the basic configuration struct used to start up an
/// adapter instance.
pub mod config;
/// This module contains code that is used to manage a single connection to a
/// BTC node.
mod connection;
/// This module contains code that is used to manage multiple connections to
/// BTC nodes.
mod connectionmanager;
mod metrics;
/// The module is responsible for awaiting messages from bitcoin peers and dispaching them
/// to the correct component.
mod router;
/// This module contains code that is used to handle interactions to connected
/// BTC streams (SOCKS and TCP).
mod rpc_server;
mod stream;
mod transaction_store;

// This module contains code that is used to return requested blocks to the Bitcoin canister.
// For security reasons, it expects the returned blocks to be in a BFS order (for example, a
// malicious fork can be prioritized by a DFS, thus potentially ignoring honest forks).
mod get_successors_handler;

use crate::{router::start_main_event_loop, stream::StreamEvent};
pub use blockchainstate::BlockchainState;
pub use get_successors_handler::GetSuccessorsHandler;
pub use rpc_server::start_grpc_server;

/// This struct is used to represent commands given to the adapter in order to interact
/// with BTC nodes.
#[derive(Clone, Eq, PartialEq, Debug)]
struct Command {
    /// This is the address of the Bitcoin node to which the message is supposed to be sent.
    /// If the address is None, then the message will be sent to all the peers.
    address: Option<SocketAddr>,
    /// This the network message to be sent to the above peer.
    message: NetworkMessage,
}

/// This enum is used to represent errors that could occur while dispatching an
/// event.
#[derive(Debug)]
enum ProcessBitcoinNetworkMessageError {
    /// This variant is used to represent when an invalid message has been
    /// received from a Bitcoin node.
    InvalidMessage,
}

/// This enum is used to represent errors that  
#[derive(Debug)]
enum ChannelError {}

/// This trait is to provide an interface so that managers can communicate to BTC nodes.
trait Channel {
    /// This method is used to send a message to a specific connection
    /// or to all connections based on the [Command](Command)'s fields.
    fn send(&mut self, command: Command) -> Result<(), ChannelError>;

    /// This method is used to retrieve a list of available connections
    /// that have completed the version handshake.
    fn available_connections(&self) -> Vec<SocketAddr>;

    /// Used to disconnect from nodes that are misbehaving.
    fn discard(&mut self, addr: &SocketAddr);
}

/// This trait provides an interface to anything that may need to react to a
/// [StreamEvent](crate::stream::StreamEvent).
trait ProcessEvent {
    /// This method is used to route an event in a component's internals and
    /// perform state updates.
    fn process_event(
        &mut self,
        event: &StreamEvent,
    ) -> Result<(), ProcessBitcoinNetworkMessageError>;
}

/// This trait provides an interface for processing messages coming from
/// bitcoin peers.
/// [StreamEvent](crate::stream::StreamEvent).
trait ProcessBitcoinNetworkMessage {
    /// This method is used to route an event in a component's internals and
    /// perform state updates.
    fn process_bitcoin_network_message(
        &mut self,
        addr: SocketAddr,
        message: &NetworkMessage,
    ) -> Result<(), ProcessBitcoinNetworkMessageError>;
}

/// Commands sent back to the router in order perform actions on the blockchain state.
#[derive(Debug)]
pub enum BlockchainManagerRequest {
    /// Inform the adapter to enqueue the next block headers into the syncing queue.
    EnqueueNewBlocksToDownload(Vec<BlockHeader>),
    /// Inform the adapter to prune the following block hashes from the cache.
    PruneBlocks(BlockHash, Vec<BlockHash>),
}

/// The transaction manager is owned by a single thread which listens on a channel
/// for TransactionManagerRequest messages and executes the corresponding method.
#[derive(Debug)]
pub enum TransactionManagerRequest {
    /// Command for executing send_transaction
    SendTransaction(Vec<u8>),
}

/// The type tracks when then adapter should become idle. The type is
/// thread-safe.
#[derive(Clone)]
pub struct AdapterState {
    /// The field contains instant of the latest received request.
    /// None means that we haven't reveived a request yet and the adapter should be in idle mode!
    ///
    /// !!! BE CAREFUL HERE !!! since the adapter should ALWAYS be idle when starting up.
    /// This is important because most subnets will have bitcoin integration disabled and we don't want
    /// to unnecessary download bitcoin data.
    /// In a previous iteration we set this value to at least 'idle_seconds' in the past on startup.
    /// This way the adapter would always be in idle when starting since 'elapsed()' is greater than 'idle_seconds'.
    /// On MacOS this approach caused issues since on MacOS Instant::now() is time since boot and when subtracting
    /// 'idle_seconds' we encountered an underflow and panicked.
    last_received_at: Arc<RwLock<Option<Instant>>>,
    /// The field contains how long the adapter should wait to before becoming idle.
    idle_seconds: u64,

    /// The field contains a watch channel that is used to wake up the adapter when it is idle.
    awake_tx: watch::Sender<()>,
}

impl AdapterState {
    /// Crates new instance of the AdapterState.
    pub fn new(idle_seconds: u64) -> Self {
        let (awake_tx, _) = watch::channel(());
        let state = Self {
            last_received_at: Arc::new(RwLock::new(None)),
            idle_seconds,
            awake_tx,
        };
        state
    }

    /// Updates the current state of the adapter given a request was received.
    pub fn received_now(&self) {
        // Instant::now() is monotonically nondecreasing clock.
        *self.last_received_at.write() = Some(Instant::now());
        // TODO: perhaps log something if this fails
        let _ = self.awake_tx.send(());
    }

    /// A future that returns when/if the adapter becomes/is awake.
    pub async fn become_awake(&self) {
        let mut awake_rx = self.awake_tx.clone().subscribe();
        loop {
            match *self.last_received_at.read() {
                Some(last) => {
                    if last.elapsed().as_secs() < self.idle_seconds {
                        return ();
                    }
                }
                // Nothing received yet still in idle from startup.
                None => {},
            };
            let _ = awake_rx.changed().await;
        }
    }

    /// A future that returns when/if the adapter becomes/is idle.
    pub async fn become_idle(&self) {
        loop {
            let mut tick_interval: tokio::time::Interval;
            match *self.last_received_at.read() {
                Some(last) => {
                    if last.elapsed().as_secs() > self.idle_seconds {
                        return ();
                    }
                    // tick again for the remaining seconds
                    tick_interval = interval(Duration::from_secs(self.idle_seconds - last.elapsed().as_secs()));
                }
                // Nothing received yet still in idle from startup.
                None => return (),
            };
            tick_interval.tick().await;
        }
    }
}

/// Starts the gRPC server and the router for handling incoming requests.
pub fn start_server(
    log: &ReplicaLogger,
    metrics_registry: &MetricsRegistry,
    rt_handle: &tokio::runtime::Handle,
    config: config::Config,
) {
    let _enter = rt_handle.enter();

    let adapter_state = AdapterState::new(config.idle_seconds);

    let (blockchain_manager_tx, blockchain_manager_rx) = channel(100);
    let blockchain_state = Arc::new(Mutex::new(BlockchainState::new(&config, metrics_registry)));
    let get_successors_handler = GetSuccessorsHandler::new(
        &config,
        // The get successor handler should be low latency, and instead of not sharing state and
        // offloading the computation to an event loop here we directly access the shared state.
        blockchain_state.clone(),
        blockchain_manager_tx,
        metrics_registry,
    );

    let (transaction_manager_tx, transaction_manager_rx) = channel(100);

    start_grpc_server(
        config.clone(),
        log.clone(),
        adapter_state.clone(),
        get_successors_handler,
        transaction_manager_tx,
        metrics_registry,
    );

    start_main_event_loop(
        &config,
        log.clone(),
        blockchain_state,
        transaction_manager_rx,
        adapter_state,
        blockchain_manager_rx,
        metrics_registry,
    );
}
