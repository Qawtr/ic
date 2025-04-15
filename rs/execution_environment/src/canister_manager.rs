use crate::as_round_instructions;
use crate::canister_settings::{validate_canister_settings, ValidatedCanisterSettings};
use crate::execution::install_code::{validate_controller, OriginalContext};
use crate::execution::{install::execute_install, upgrade::execute_upgrade};
use crate::execution_environment::{
    CompilationCostHandling, RoundContext, RoundCounters, RoundLimits,
};
use crate::{
    canister_settings::CanisterSettings,
    hypervisor::Hypervisor,
    types::{IngressResponse, Response},
    util::GOVERNANCE_CANISTER_ID,
};
use ic_config::embedders::Config as EmbeddersConfig;
use ic_config::flag_status::FlagStatus;
use ic_cycles_account_manager::{CyclesAccountManager, ResourceSaturation};
use ic_embedders::{
    wasm_utils::decoding::decode_wasm, wasmtime_embedder::system_api::ExecutionParameters,
};
use ic_error_types::{ErrorCode, RejectCode, UserError};
use ic_interfaces::execution_environment::{IngressHistoryWriter, SubnetAvailableMemory};
use ic_logger::{error, fatal, info, ReplicaLogger};
use ic_management_canister_types_private::{
    CanisterChangeDetails, CanisterChangeOrigin, CanisterInstallModeV2, CanisterSnapshotDataKind,
    CanisterSnapshotResponse, CanisterStatusResultV2, CanisterStatusType, ChunkHash, GlobalTimer,
    Method as Ic00Method, ReadCanisterSnapshotDataResponse, ReadCanisterSnapshotMetadataResponse,
    StoredChunksReply, UploadChunkReply,
};
use ic_registry_provisional_whitelist::ProvisionalWhitelist;
use ic_replicated_state::canister_state::system_state::wasm_chunk_store::{
    WasmChunkHash, CHUNK_SIZE,
};
use ic_replicated_state::canister_state::WASM_PAGE_SIZE_IN_BYTES;
use ic_replicated_state::{
    canister_snapshots::CanisterSnapshot,
    canister_state::{
        execution_state::Memory,
        execution_state::WasmExecutionMode,
        system_state::{
            wasm_chunk_store::{self, WasmChunkStore},
            CyclesUseCase, ReservationError,
        },
        NextExecution,
    },
    metadata_state::subnet_call_context_manager::InstallCodeCallId,
    page_map::PageAllocatorFileDescriptor,
    CallOrigin, CanisterState, MessageMemoryUsage, NetworkTopology, ReplicatedState,
    SchedulerState, SystemState,
};
use ic_types::{
    ingress::{IngressState, IngressStatus},
    messages::{
        CanisterCall, Payload, RejectContext, Response as CanisterResponse, SignedIngressContent,
        StopCanisterContext,
    },
    nominal_cycles::NominalCycles,
    CanisterId, CanisterTimer, ComputeAllocation, Cycles, MemoryAllocation, NumBytes,
    NumInstructions, PrincipalId, SnapshotId, SubnetId, Time,
};
use ic_wasm_types::WasmHash;
use num_traits::{SaturatingAdd, SaturatingSub};
use prometheus::IntCounter;
use std::path::PathBuf;
use std::{convert::TryFrom, str::FromStr, sync::Arc};

use types::*;
pub(crate) mod types;

/// Maximum binary slice size allowed per single message download.
const MAX_SLICE_SIZE_BYTES: u64 = 2_000_000;

/// The entity responsible for managing canisters (creation, installing, etc.)
pub(crate) struct CanisterManager {
    hypervisor: Arc<Hypervisor>,
    log: ReplicaLogger,
    config: CanisterMgrConfig,
    cycles_account_manager: Arc<CyclesAccountManager>,
    ingress_history_writer: Arc<dyn IngressHistoryWriter<State = ReplicatedState>>,
    fd_factory: Arc<dyn PageAllocatorFileDescriptor>,
}

impl CanisterManager {
    pub(crate) fn new(
        hypervisor: Arc<Hypervisor>,
        log: ReplicaLogger,
        config: CanisterMgrConfig,
        cycles_account_manager: Arc<CyclesAccountManager>,
        ingress_history_writer: Arc<dyn IngressHistoryWriter<State = ReplicatedState>>,
        fd_factory: Arc<dyn PageAllocatorFileDescriptor>,
    ) -> Self {
        CanisterManager {
            hypervisor,
            log,
            config,
            cycles_account_manager,
            ingress_history_writer,
            fd_factory,
        }
    }

    /// Checks if a given ingress message directed to the management canister
    /// should be accepted or not.
    pub(crate) fn should_accept_ingress_message(
        &self,
        state: Arc<ReplicatedState>,
        provisional_whitelist: &ProvisionalWhitelist,
        ingress: &SignedIngressContent,
        effective_canister_id: Option<CanisterId>,
    ) -> Result<(), UserError> {
        let method_name = ingress.method_name();
        let sender = ingress.sender();
        let method = Ic00Method::from_str(ingress.method_name());
        // The message is targeted towards the management canister. The
        // actual type of the method will determine if the message should be
        // accepted or not.
        match method {
            // The method is either invalid or it is of a type that users
            // are not allowed to send.
            Err(_)
            | Ok(Ic00Method::CreateCanister)
            | Ok(Ic00Method::CanisterInfo)
            | Ok(Ic00Method::ECDSAPublicKey)
            | Ok(Ic00Method::SetupInitialDKG)
            | Ok(Ic00Method::SignWithECDSA)
            | Ok(Ic00Method::ComputeInitialIDkgDealings)
            | Ok(Ic00Method::ReshareChainKey)
            | Ok(Ic00Method::SchnorrPublicKey)
            | Ok(Ic00Method::SignWithSchnorr)
            | Ok(Ic00Method::VetKdPublicKey)
            | Ok(Ic00Method::VetKdDeriveKey)
            // "DepositCycles" can be called by anyone however as ingress message
            // cannot carry cycles, it does not make sense to allow them from users.
            | Ok(Ic00Method::DepositCycles)
            | Ok(Ic00Method::HttpRequest)
            // Nobody pays for `raw_rand`, so this cannot be used via ingress messages
            | Ok(Ic00Method::RawRand)
            // Bitcoin messages require cycles, so we reject all ingress messages.
            | Ok(Ic00Method::BitcoinGetBalance)
            | Ok(Ic00Method::BitcoinGetUtxos)
            | Ok(Ic00Method::BitcoinGetBlockHeaders)
            | Ok(Ic00Method::BitcoinSendTransaction)
            | Ok(Ic00Method::BitcoinSendTransactionInternal)
            | Ok(Ic00Method::BitcoinGetCurrentFeePercentiles)
            | Ok(Ic00Method::NodeMetricsHistory)
            | Ok(Ic00Method::SubnetInfo) => Err(UserError::new(
                ErrorCode::CanisterRejectedMessage,
                format!("Only canisters can call ic00 method {}", method_name),
            )),

            // These methods are only valid if they are sent by the controller
            // of the canister. We assume that the canister always wants to
            // accept messages from its controller.
            Ok(Ic00Method::CanisterStatus)
            | Ok(Ic00Method::StartCanister)
            | Ok(Ic00Method::UninstallCode)
            | Ok(Ic00Method::StopCanister)
            | Ok(Ic00Method::DeleteCanister)
            | Ok(Ic00Method::UpdateSettings)
            | Ok(Ic00Method::InstallCode)
            | Ok(Ic00Method::InstallChunkedCode)
            | Ok(Ic00Method::UploadChunk)
            | Ok(Ic00Method::StoredChunks)
            | Ok(Ic00Method::ClearChunkStore)
            | Ok(Ic00Method::TakeCanisterSnapshot)
            | Ok(Ic00Method::LoadCanisterSnapshot)
            | Ok(Ic00Method::ListCanisterSnapshots)
            | Ok(Ic00Method::DeleteCanisterSnapshot)
            | Ok(Ic00Method::ReadCanisterSnapshotMetadata)
            | Ok(Ic00Method::ReadCanisterSnapshotData)
            | Ok(Ic00Method::UploadCanisterSnapshotMetadata)
            | Ok(Ic00Method::UploadCanisterSnapshotData) => {
                match effective_canister_id {
                    Some(canister_id) => {
                        let canister = state.canister_state(&canister_id).ok_or_else(|| UserError::new(
                            ErrorCode::CanisterNotFound,
                            format!("Canister {} not found", canister_id),
                        ))?;
                        match canister.controllers().contains(&sender.get()) {
                            true => Ok(()),
                            false => Err(UserError::new(
                                ErrorCode::CanisterInvalidController,
                                format!(
                                    "Only controllers of canister {} can call ic00 method {}",
                                    canister_id, method_name,
                                ),
                            )),
                        }
                    },
                    None => Err(UserError::new(
                        ErrorCode::InvalidManagementPayload,
                        format!("Failed to decode payload for ic00 method: {}", method_name),
                    )),
                }
            },

            Ok(Ic00Method::FetchCanisterLogs) => Err(UserError::new(
                ErrorCode::CanisterRejectedMessage,
                format!(
                    "{} API is only accessible in non-replicated mode",
                    Ic00Method::FetchCanisterLogs
                ),
            )),

            Ok(Ic00Method::ProvisionalCreateCanisterWithCycles)
            | Ok(Ic00Method::BitcoinGetSuccessors)
            | Ok(Ic00Method::ProvisionalTopUpCanister) => {
                if provisional_whitelist.contains(sender.get_ref()) {
                    Ok(())
                } else {
                    Err(UserError::new(
                        ErrorCode::CanisterRejectedMessage,
                        format!("Caller {} is not allowed to call ic00 method {}", sender, method_name)
                    ))
                }
            },
        }
    }

    fn validate_settings_for_canister_creation(
        &self,
        settings: CanisterSettings,
        subnet_compute_allocation_usage: u64,
        subnet_available_memory: &SubnetAvailableMemory,
        subnet_memory_saturation: &ResourceSaturation,
        canister_cycles_balance: Cycles,
        subnet_size: usize,
    ) -> Result<ValidatedCanisterSettings, CanisterManagerError> {
        validate_canister_settings(
            settings,
            NumBytes::new(0),
            MessageMemoryUsage::ZERO,
            MemoryAllocation::BestEffort,
            subnet_available_memory,
            subnet_memory_saturation,
            ComputeAllocation::zero(),
            subnet_compute_allocation_usage,
            self.config.compute_capacity,
            self.config.max_controllers,
            self.config.default_freeze_threshold,
            canister_cycles_balance,
            &self.cycles_account_manager,
            subnet_size,
            Cycles::zero(),
            None,
        )
    }

    /// Applies the requested settings on the canister.
    /// Note: Called only after validating the settings.
    /// Keep this function in sync with `validate_canister_settings()`.
    fn do_update_settings(
        &self,
        settings: ValidatedCanisterSettings,
        canister: &mut CanisterState,
    ) {
        // Note: At this point, the settings are validated.
        if let Some(controllers) = settings.controllers() {
            canister.system_state.controllers.clear();
            for principal in controllers {
                canister.system_state.controllers.insert(principal);
            }
        }
        if let Some(compute_allocation) = settings.compute_allocation() {
            canister.scheduler_state.compute_allocation = compute_allocation;
        }
        if let Some(memory_allocation) = settings.memory_allocation() {
            if let MemoryAllocation::Reserved(new_bytes) = memory_allocation {
                let memory_usage = canister.memory_usage();
                if new_bytes < memory_usage {
                    // This case is unreachable because the canister settings should have been validated.
                    error!(
                        self.log,
                        "[EXC-BUG]: Canister {}: unreachable code in update settings: \
                        memory allocation {} is lower than memory usage {}.",
                        canister.canister_id(),
                        new_bytes,
                        memory_usage,
                    );
                }
            }
            canister.system_state.memory_allocation = memory_allocation;
        }
        if let Some(wasm_memory_threshold) = settings.wasm_memory_threshold() {
            canister.system_state.wasm_memory_threshold = wasm_memory_threshold;
        }
        if let Some(limit) = settings.reserved_cycles_limit() {
            canister.system_state.set_reserved_balance_limit(limit);
        }
        canister
            .system_state
            .reserve_cycles(settings.reservation_cycles())
            .expect(
                "Reserving cycles should succeed because \
                    the canister settings have been validated.",
            );
        if let Some(freezing_threshold) = settings.freezing_threshold() {
            canister.system_state.freeze_threshold = freezing_threshold;
        }
        if let Some(log_visibility) = settings.log_visibility() {
            canister.system_state.log_visibility = log_visibility.clone();
        }
        if let Some(wasm_memory_limit) = settings.wasm_memory_limit() {
            canister.system_state.wasm_memory_limit = Some(wasm_memory_limit);
        }
    }

    /// Tries to apply the requested settings on the canister identified by
    /// `canister_id`.
    pub(crate) fn update_settings(
        &self,
        timestamp_nanos: Time,
        origin: CanisterChangeOrigin,
        settings: CanisterSettings,
        canister: &mut CanisterState,
        round_limits: &mut RoundLimits,
        subnet_memory_saturation: ResourceSaturation,
        subnet_size: usize,
    ) -> Result<(), CanisterManagerError> {
        let sender = origin.origin();

        validate_controller(canister, &sender)?;

        let validated_settings = validate_canister_settings(
            settings,
            canister.memory_usage(),
            canister.message_memory_usage(),
            canister.memory_allocation(),
            &round_limits.subnet_available_memory,
            &subnet_memory_saturation,
            canister.compute_allocation(),
            round_limits.compute_allocation_used,
            self.config.compute_capacity,
            self.config.max_controllers,
            canister.system_state.freeze_threshold,
            canister.system_state.balance(),
            &self.cycles_account_manager,
            subnet_size,
            canister.system_state.reserved_balance(),
            canister.system_state.reserved_balance_limit(),
        )?;

        let is_controllers_change = validated_settings.controllers().is_some();

        let old_usage = canister.memory_usage();
        let old_mem = canister.memory_allocation().allocated_bytes(old_usage);
        let old_compute_allocation = canister.scheduler_state.compute_allocation.as_percent();

        self.do_update_settings(validated_settings, canister);

        let new_compute_allocation = canister.scheduler_state.compute_allocation.as_percent();
        if old_compute_allocation < new_compute_allocation {
            round_limits.compute_allocation_used = round_limits
                .compute_allocation_used
                .saturating_add(new_compute_allocation - old_compute_allocation);
        } else {
            round_limits.compute_allocation_used = round_limits
                .compute_allocation_used
                .saturating_sub(old_compute_allocation - new_compute_allocation);
        }

        let new_usage = old_usage;
        let new_mem = canister.memory_allocation().allocated_bytes(new_usage);
        if new_mem >= old_mem {
            // Settings were validated before so this should always succeed.
            round_limits
                .subnet_available_memory
                .try_decrement(new_mem - old_mem, NumBytes::from(0), NumBytes::from(0))
                .expect("Error: Cannot fail to decrement SubnetAvailableMemory after validating the canister's settings");
        } else {
            round_limits.subnet_available_memory.increment(
                old_mem - new_mem,
                NumBytes::from(0),
                NumBytes::from(0),
            );
        }

        canister.system_state.canister_version += 1;
        if is_controllers_change {
            let new_controllers = canister.system_state.controllers.iter().copied().collect();
            canister.system_state.add_canister_change(
                timestamp_nanos,
                origin,
                CanisterChangeDetails::controllers_change(new_controllers),
            );
        }

        Ok(())
    }

    /// Creates a new canister and inserts it into `ReplicatedState`.
    ///
    /// Returns the auto-generated id the new canister that has been created.
    pub(crate) fn create_canister(
        &self,
        origin: CanisterChangeOrigin,
        sender_subnet_id: SubnetId,
        cycles: Cycles,
        mut settings: CanisterSettings,
        max_number_of_canisters: u64,
        state: &mut ReplicatedState,
        subnet_size: usize,
        round_limits: &mut RoundLimits,
        subnet_memory_saturation: ResourceSaturation,
        canister_creation_error: &IntCounter,
    ) -> (Result<CanisterId, CanisterManagerError>, Cycles) {
        // Creating a canister is possible only in the following cases:
        // 1. sender is on NNS => it can create canister on any subnet
        // 2. sender is not NNS => can create canister only if sender is on
        // same subnet.
        if sender_subnet_id != state.metadata.network_topology.nns_subnet_id
            && sender_subnet_id != self.config.own_subnet_id
        {
            return (
                Err(CanisterManagerError::InvalidSenderSubnet(sender_subnet_id)),
                cycles,
            );
        }

        let fee = self
            .cycles_account_manager
            .canister_creation_fee(subnet_size);
        if cycles < fee {
            return (
                Err(CanisterManagerError::CreateCanisterNotEnoughCycles {
                    sent: cycles,
                    required: fee,
                }),
                cycles,
            );
        }

        // Set the field to the default value if it is empty.
        settings
            .reserved_cycles_limit
            .get_or_insert_with(|| self.cycles_account_manager.default_reserved_balance_limit());

        settings
            .wasm_memory_limit
            .get_or_insert(self.config.default_wasm_memory_limit);

        // Validate settings before `create_canister_helper` applies them
        match self.validate_settings_for_canister_creation(
            settings,
            round_limits.compute_allocation_used,
            &round_limits.subnet_available_memory,
            &subnet_memory_saturation,
            cycles - fee,
            subnet_size,
        ) {
            Err(err) => (Err(err), cycles),
            Ok(validate_settings) => {
                let canister_id = match self.create_canister_helper(
                    origin,
                    cycles,
                    fee,
                    validate_settings,
                    max_number_of_canisters,
                    state,
                    round_limits,
                    None,
                    canister_creation_error,
                ) {
                    Ok(canister_id) => canister_id,
                    Err(err) => return (Err(err), cycles),
                };
                (Ok(canister_id), Cycles::zero())
            }
        }
    }

    /// Checks if the given wasm module is a Wasm64 module.
    /// This is solely for the purpose of install code, when at the replica level
    /// we don't know yet if the module is Wasm32/64 and we need to prepay accordingly.
    /// In case of errors, we simply return false, assuming Wasm32.
    /// The errors will be caught and handled by the sandbox later.
    fn check_if_wasm64_module(&self, wasm_module_source: WasmSource) -> bool {
        let wasm_module = match wasm_module_source.into_canister_module() {
            Ok(wasm_module) => wasm_module,
            Err(_err) => {
                return false;
            }
        };

        let decoded_wasm_module = match decode_wasm(
            EmbeddersConfig::new().wasm_max_size,
            Arc::new(wasm_module.as_slice().to_vec()),
        ) {
            Ok(decoded_wasm_module) => decoded_wasm_module,
            Err(_err) => {
                return false;
            }
        };

        let parser = wasmparser::Parser::new(0);
        for section in parser.parse_all(decoded_wasm_module.as_slice()).flatten() {
            if let wasmparser::Payload::MemorySection(reader) = section {
                if let Some(memory) = reader.into_iter().flatten().next() {
                    return memory.memory64;
                }
            }
        }
        false
    }

    /// Installs code to a canister.
    ///
    /// Only the controller of the canister can install code.
    ///
    /// There are three modes of installation that are supported:
    ///
    /// 1. `CanisterInstallModeV2::Install`
    ///    Used for installing code on an empty canister.
    ///
    /// 2. `CanisterInstallModeV2::Reinstall`
    ///    Used for installing code on a _non-empty_ canister. All existing
    ///    state in the canister is cleared.
    ///
    /// 3. `CanisterInstallModeV2::Upgrade`
    ///    Used for upgrading a canister while providing a mechanism to
    ///    preserve its state.
    ///
    /// This function is atomic. Either all of its subroutines succeed,
    /// or the changes made to old_canister are reverted to the state
    /// from before execution of the first one.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn install_code_dts(
        &self,
        context: InstallCodeContext,
        message: CanisterCall,
        call_id: InstallCodeCallId,
        prepaid_execution_cycles: Option<Cycles>,
        mut canister: CanisterState,
        time: Time,
        canister_layout_path: PathBuf,
        network_topology: &NetworkTopology,
        execution_parameters: ExecutionParameters,
        round_limits: &mut RoundLimits,
        compilation_cost_handling: CompilationCostHandling,
        round_counters: RoundCounters,
        subnet_size: usize,
        log_dirty_pages: FlagStatus,
    ) -> DtsInstallCodeResult {
        if let Err(err) = validate_controller(&canister, &context.sender()) {
            return DtsInstallCodeResult::Finished {
                canister,
                message,
                call_id,
                instructions_used: NumInstructions::from(0),
                result: Err(err),
            };
        }

        let wasm_execution_mode = WasmExecutionMode::from_is_wasm64(
            self.check_if_wasm64_module(context.wasm_source.clone()),
        );

        let prepaid_execution_cycles = match prepaid_execution_cycles {
            Some(prepaid_execution_cycles) => prepaid_execution_cycles,
            None => {
                let memory_usage = canister.memory_usage();
                let message_memory_usage = canister.message_memory_usage();
                let reveal_top_up = canister.controllers().contains(message.sender());

                match self.cycles_account_manager.prepay_execution_cycles(
                    &mut canister.system_state,
                    memory_usage,
                    message_memory_usage,
                    execution_parameters.compute_allocation,
                    execution_parameters.instruction_limits.message(),
                    subnet_size,
                    reveal_top_up,
                    wasm_execution_mode,
                ) {
                    Ok(cycles) => cycles,
                    Err(err) => {
                        return DtsInstallCodeResult::Finished {
                            canister,
                            message,
                            call_id,
                            instructions_used: NumInstructions::from(0),
                            result: Err(CanisterManagerError::InstallCodeNotEnoughCycles(err)),
                        };
                    }
                }
            }
        };

        let original: OriginalContext = OriginalContext {
            execution_parameters,
            mode: context.mode,
            canister_layout_path,
            config: self.config.clone(),
            message,
            call_id,
            prepaid_execution_cycles,
            time,
            compilation_cost_handling,
            subnet_size,
            requested_compute_allocation: context.compute_allocation,
            requested_memory_allocation: context.memory_allocation,
            sender: context.sender(),
            canister_id: canister.canister_id(),
            log_dirty_pages,
            wasm_execution_mode,
        };

        let round = RoundContext {
            network_topology,
            hypervisor: &self.hypervisor,
            cycles_account_manager: &self.cycles_account_manager,
            counters: round_counters,
            log: &self.log,
            time,
        };

        match context.mode {
            CanisterInstallModeV2::Install | CanisterInstallModeV2::Reinstall => {
                execute_install(context, canister, original, round.clone(), round_limits)
            }
            CanisterInstallModeV2::Upgrade(..) => {
                execute_upgrade(context, canister, original, round.clone(), round_limits)
            }
        }
    }

    /// Uninstalls code from a canister.
    ///
    /// See https://internetcomputer.org/docs/current/references/ic-interface-spec#ic-uninstall_code
    pub(crate) fn uninstall_code(
        &self,
        origin: CanisterChangeOrigin,
        canister_id: CanisterId,
        state: &mut ReplicatedState,
        canister_not_found_error: &IntCounter,
    ) -> Result<(), CanisterManagerError> {
        let sender = origin.origin();
        let time = state.time();
        let canister = match state.canister_state_mut(&canister_id) {
            Some(canister) => canister,
            None => return Err(CanisterManagerError::CanisterNotFound(canister_id)),
        };

        // Skip the controller validation if the sender is the governance
        // canister. The governance canister can forcefully
        // uninstall the code of any canister.
        if sender != GOVERNANCE_CANISTER_ID.get() {
            validate_controller(canister, &sender)?
        }

        let rejects = uninstall_canister(
            &self.log,
            canister,
            time,
            AddCanisterChangeToHistory::Yes(origin),
            Arc::clone(&self.fd_factory),
        );
        crate::util::process_responses(
            rejects,
            state,
            Arc::clone(&self.ingress_history_writer),
            self.log.clone(),
            canister_not_found_error,
        );
        Ok(())
    }

    /// Signals a canister to stop.
    ///
    /// If the canister is running, then the canister is marked as "stopping".
    /// Stopping is meant to be an ephemeral state where the canister has the
    /// opportunity to close its call contexts before fully stopping. The stop
    /// message is saved in the canister's status so that, at a later point, the
    /// scheduler can respond to that message when the canister is fully
    /// stopped.
    ///
    /// If the canister is in the stopping state, then the stop message is
    /// appended to the canister's status. At a later point when the canister is
    /// ready to be fully stopped, the scheduler will respond to this message.
    ///
    /// If the canister is already stopped, then this function is a no-op.
    pub(crate) fn stop_canister(
        &self,
        canister_id: CanisterId,
        mut stop_context: StopCanisterContext,
        state: &mut ReplicatedState,
    ) -> StopCanisterResult {
        let canister = match state.canister_state_mut(&canister_id) {
            None => {
                return StopCanisterResult::Failure {
                    error: CanisterManagerError::CanisterNotFound(canister_id),
                    cycles_to_return: stop_context.take_cycles(),
                }
            }
            Some(canister) => canister,
        };

        let result = match validate_controller(canister, stop_context.sender()) {
            Err(err) => StopCanisterResult::Failure {
                error: err,
                cycles_to_return: stop_context.take_cycles(),
            },

            Ok(()) => match canister.system_state.begin_stopping(stop_context) {
                Some(mut stop_context) => StopCanisterResult::AlreadyStopped {
                    cycles_to_return: stop_context.take_cycles(),
                },
                None => StopCanisterResult::RequestAccepted,
            },
        };
        canister.system_state.canister_version += 1;
        result
    }

    /// Signals a canister to start.
    ///
    /// If the canister is stopped, then the canister is immediately
    /// transitioned into the "running" state.
    ///
    /// If the canister is already running, this operation is a no-op.
    ///
    /// If the canister is in the process of being stopped (i.e stopping), then
    /// the canister is transitioned back into a running state and the
    /// `stop_contexts` that were used for stopping the canister are
    /// returned.
    pub(crate) fn start_canister(
        &self,
        sender: PrincipalId,
        canister: &mut CanisterState,
    ) -> Result<Vec<StopCanisterContext>, CanisterManagerError> {
        validate_controller(canister, &sender)?;

        let stop_contexts = canister.system_state.start_canister();
        canister.system_state.canister_version += 1;

        Ok(stop_contexts)
    }

    /// Fetches the current status of the canister.
    pub(crate) fn get_canister_status(
        &self,
        sender: PrincipalId,
        canister: &mut CanisterState,
        subnet_size: usize,
    ) -> Result<CanisterStatusResultV2, CanisterManagerError> {
        // Skip the controller check if the canister itself is requesting its
        // own status, as the canister is considered in the same trust domain.
        if sender != canister.canister_id().get() {
            validate_controller(canister, &sender)?
        }

        let controller = canister.system_state.controller();
        let controllers = canister
            .controllers()
            .iter()
            .copied()
            .collect::<Vec<PrincipalId>>();

        let canister_memory_usage = canister.memory_usage();
        let canister_wasm_memory_usage = canister.wasm_memory_usage();
        let canister_stable_memory_usage = canister.stable_memory_usage();
        let canister_global_memory_usage = canister.global_memory_usage();
        let canister_wasm_binary_memory_usage = canister.wasm_binary_memory_usage();
        let canister_custom_sections_memory_usage = canister.wasm_custom_sections_memory_usage();
        let canister_history_memory_usage = canister.canister_history_memory_usage();
        let canister_wasm_chunk_store_memory_usage = canister.wasm_chunk_store_memory_usage();
        let canister_snapshots_memory_usage = canister.snapshots_memory_usage();
        let canister_message_memory_usage = canister.message_memory_usage();
        let compute_allocation = canister.scheduler_state.compute_allocation;
        let memory_allocation = canister.memory_allocation();
        let freeze_threshold = canister.system_state.freeze_threshold;
        let reserved_cycles_limit = canister.system_state.reserved_balance_limit();
        let log_visibility = canister.system_state.log_visibility.clone();
        let wasm_memory_limit = canister.system_state.wasm_memory_limit;
        let wasm_memory_threshold = canister.system_state.wasm_memory_threshold;

        Ok(CanisterStatusResultV2::new(
            canister.status(),
            canister
                .execution_state
                .as_ref()
                .map(|es| es.wasm_binary.binary.module_hash().to_vec()),
            *controller,
            controllers,
            canister_memory_usage,
            canister_wasm_memory_usage,
            canister_stable_memory_usage,
            canister_global_memory_usage,
            canister_wasm_binary_memory_usage,
            canister_custom_sections_memory_usage,
            canister_history_memory_usage,
            canister_wasm_chunk_store_memory_usage,
            canister_snapshots_memory_usage,
            canister.system_state.balance().get(),
            compute_allocation.as_percent(),
            Some(memory_allocation.bytes().get()),
            freeze_threshold.get(),
            reserved_cycles_limit.map(|x| x.get()),
            log_visibility,
            self.cycles_account_manager
                .idle_cycles_burned_rate(
                    memory_allocation,
                    canister_memory_usage,
                    canister_message_memory_usage,
                    compute_allocation,
                    subnet_size,
                )
                .get(),
            canister.system_state.reserved_balance().get(),
            canister.scheduler_state.total_query_stats.num_calls,
            canister.scheduler_state.total_query_stats.num_instructions,
            canister
                .scheduler_state
                .total_query_stats
                .ingress_payload_size,
            canister
                .scheduler_state
                .total_query_stats
                .egress_payload_size,
            wasm_memory_limit.map(|x| x.get()),
            wasm_memory_threshold.get(),
        ))
    }

    /// Permanently deletes a canister from `ReplicatedState`.
    ///
    /// The canister must be `Stopped` and only the controller of the canister
    /// can delete it. The controller must be a canister and the canister
    /// cannot be its own controller.
    ///
    /// Any remaining cycles in the canister are discarded.
    ///
    /// #Errors
    /// CanisterManagerError::DeleteCanisterSelf is the canister attempts to
    /// delete itself.
    pub(crate) fn delete_canister(
        &self,
        sender: PrincipalId,
        canister_id_to_delete: CanisterId,
        state: &mut ReplicatedState,
    ) -> Result<(), CanisterManagerError> {
        if let Ok(canister_id) = CanisterId::try_from(sender) {
            if canister_id == canister_id_to_delete {
                // A canister cannot delete itself.
                return Err(CanisterManagerError::DeleteCanisterSelf(canister_id));
            }
        }

        let canister_to_delete = self.validate_canister_exists(state, canister_id_to_delete)?;

        // Validate the request is from the controller.
        validate_controller(canister_to_delete, &sender)?;

        self.validate_canister_is_stopped(canister_to_delete)?;

        if canister_to_delete.has_input() || canister_to_delete.has_output() {
            return Err(CanisterManagerError::DeleteCanisterQueueNotEmpty(
                canister_id_to_delete,
            ));
        }

        // When a canister is deleted:
        // - its state is permanently deleted, and
        // - its cycles are discarded.

        // Take out the canister from `ReplicatedState`.
        let canister_to_delete = state.take_canister_state(&canister_id_to_delete).unwrap();
        state
            .canister_snapshots
            .delete_snapshots(canister_to_delete.canister_id());

        // Leftover cycles in the balance are considered `consumed`.
        let leftover_cycles = NominalCycles::from(canister_to_delete.system_state.balance());
        let consumed_cycles_by_canister_to_delete = leftover_cycles
            + canister_to_delete
                .system_state
                .canister_metrics
                .consumed_cycles;

        state
            .metadata
            .subnet_metrics
            .observe_consumed_cycles_with_use_case(
                CyclesUseCase::DeletedCanisters,
                leftover_cycles,
            );

        state
            .metadata
            .subnet_metrics
            .consumed_cycles_by_deleted_canisters += consumed_cycles_by_canister_to_delete;

        for (use_case, cycles) in canister_to_delete
            .system_state
            .canister_metrics
            .get_consumed_cycles_by_use_cases()
            .iter()
        {
            state
                .metadata
                .subnet_metrics
                .observe_consumed_cycles_with_use_case(*use_case, *cycles);
        }

        // The canister has now been removed from `ReplicatedState` and is dropped
        // once the function is out of scope.
        Ok(())
    }

    /// Creates a new canister with the cycles amount specified and inserts it
    /// into `ReplicatedState`.
    ///
    /// Note that this method is meant to only be invoked in local development
    /// by a list of whitelisted principals.
    ///
    /// Returns the auto-generated id the new canister that has been created.
    pub(crate) fn create_canister_with_cycles(
        &self,
        origin: CanisterChangeOrigin,
        cycles_amount: Option<u128>,
        mut settings: CanisterSettings,
        specified_id: Option<PrincipalId>,
        state: &mut ReplicatedState,
        provisional_whitelist: &ProvisionalWhitelist,
        max_number_of_canisters: u64,
        round_limits: &mut RoundLimits,
        subnet_memory_saturation: ResourceSaturation,
        subnet_size: usize,
        canister_creation_error: &IntCounter,
    ) -> Result<CanisterId, CanisterManagerError> {
        let sender = origin.origin();

        if !provisional_whitelist.contains(&sender) {
            return Err(CanisterManagerError::SenderNotInWhitelist(sender));
        }

        let cycles = match cycles_amount {
            Some(cycles_amount) => Cycles::from(cycles_amount),
            None => self.config.default_provisional_cycles_balance,
        };

        // Set the field to the default value if it is empty.
        settings
            .reserved_cycles_limit
            .get_or_insert_with(|| self.cycles_account_manager.default_reserved_balance_limit());

        // Validate settings before `create_canister_helper` applies them
        // No creation fee applied.
        match self.validate_settings_for_canister_creation(
            settings,
            round_limits.compute_allocation_used,
            &round_limits.subnet_available_memory,
            &subnet_memory_saturation,
            cycles,
            subnet_size,
        ) {
            Err(err) => Err(err),
            Ok(validated_settings) => self.create_canister_helper(
                origin,
                cycles,
                Cycles::new(0),
                validated_settings,
                max_number_of_canisters,
                state,
                round_limits,
                specified_id,
                canister_creation_error,
            ),
        }
    }

    /// Validates specified ID is available for use.
    ///
    /// It must be used in in the context of provisional create canister flow when a specified ID is provided.
    ///
    /// Returns `Err` iff the `specified_id` is not valid.
    fn validate_specified_id(
        &self,
        state: &mut ReplicatedState,
        specified_id: PrincipalId,
    ) -> Result<CanisterId, CanisterManagerError> {
        let new_canister_id = CanisterId::unchecked_from_principal(specified_id);

        if state.canister_states.contains_key(&new_canister_id) {
            return Err(CanisterManagerError::CanisterAlreadyExists(new_canister_id));
        }

        if state
            .metadata
            .network_topology
            .routing_table
            .route(specified_id)
            == Some(state.metadata.own_subnet_id)
        {
            Ok(new_canister_id)
        } else {
            Err(CanisterManagerError::CanisterNotHostedBySubnet {
                message: format!(
                    "Specified CanisterId {} is not hosted by subnet {}.",
                    specified_id, state.metadata.own_subnet_id
                ),
            })
        }
    }

    fn create_canister_helper(
        &self,
        origin: CanisterChangeOrigin,
        cycles: Cycles,
        creation_fee: Cycles,
        settings: ValidatedCanisterSettings,
        max_number_of_canisters: u64,
        state: &mut ReplicatedState,
        round_limits: &mut RoundLimits,
        specified_id: Option<PrincipalId>,
        canister_creation_error: &IntCounter,
    ) -> Result<CanisterId, CanisterManagerError> {
        let sender = origin.origin();

        // A value of 0 is equivalent to setting no limit.
        // See documentation of `SubnetRecord` for the semantics of `max_number_of_canisters`.
        if max_number_of_canisters > 0 && state.num_canisters() as u64 >= max_number_of_canisters {
            return Err(CanisterManagerError::MaxNumberOfCanistersReached {
                subnet_id: self.config.own_subnet_id,
                max_number_of_canisters,
            });
        }

        let new_canister_id = match specified_id {
            Some(spec_id) => self.validate_specified_id(state, spec_id)?,

            None => self.generate_new_canister_id(state, canister_creation_error)?,
        };

        // Canister id available. Create the new canister.
        let mut system_state = SystemState::new_running(
            new_canister_id,
            sender,
            cycles,
            self.config.default_freeze_threshold,
            Arc::clone(&self.fd_factory),
        );

        system_state.remove_cycles(creation_fee, CyclesUseCase::CanisterCreation);
        let scheduler_state = SchedulerState::new(state.metadata.batch_time);
        let mut new_canister = CanisterState::new(system_state, None, scheduler_state);

        self.do_update_settings(settings, &mut new_canister);
        let new_usage = new_canister.memory_usage();
        let new_mem = new_canister
            .system_state
            .memory_allocation
            .bytes()
            .max(new_usage);

        // settings were validated before so this should always succeed
        round_limits
            .subnet_available_memory
            .try_decrement(new_mem, NumBytes::from(0), NumBytes::from(0))
            .expect("Error: Cannot fail to decrement SubnetAvailableMemory after validating canister's settings");

        round_limits.compute_allocation_used = round_limits
            .compute_allocation_used
            .saturating_add(new_canister.scheduler_state.compute_allocation.as_percent());

        let controllers = new_canister
            .system_state
            .controllers
            .iter()
            .copied()
            .collect();
        new_canister.system_state.add_canister_change(
            state.time(),
            origin,
            CanisterChangeDetails::canister_creation(controllers),
        );

        // Add new canister to the replicated state.
        state.put_canister_state(new_canister);

        info!(
            self.log,
            "Canister {} created canister {} with {} initial balance on subnet {}.",
            sender,
            new_canister_id.get(),
            cycles,
            self.config.own_subnet_id.get()
        );

        Ok(new_canister_id)
    }

    /// Adds cycles to the canister.
    pub(crate) fn add_cycles(
        &self,
        sender: PrincipalId,
        cycles_amount: Option<u128>,
        canister: &mut CanisterState,
        provisional_whitelist: &ProvisionalWhitelist,
    ) -> Result<(), CanisterManagerError> {
        if !provisional_whitelist.contains(&sender) {
            return Err(CanisterManagerError::SenderNotInWhitelist(sender));
        }

        let cycles_amount = match cycles_amount {
            Some(cycles_amount) => Cycles::from(cycles_amount),
            None => self.config.default_provisional_cycles_balance,
        };

        canister
            .system_state
            .add_cycles(cycles_amount, CyclesUseCase::NonConsumed);

        Ok(())
    }

    fn validate_canister_is_stopped(
        &self,
        canister: &CanisterState,
    ) -> Result<(), CanisterManagerError> {
        if canister.status() != CanisterStatusType::Stopped {
            return Err(CanisterManagerError::DeleteCanisterNotStopped(
                canister.canister_id(),
            ));
        }
        Ok(())
    }

    /// Generates a new canister ID.
    ///
    /// Returns `Err` if the subnet can generate no more canister IDs; or a canister
    /// with the newly generated ID already exists.
    //
    // WARNING!!! If you change the logic here, please ensure that the sequence
    // of NNS canister ids as defined in nns/constants/src/lib.rs are also
    // updated.
    fn generate_new_canister_id(
        &self,
        state: &mut ReplicatedState,
        canister_creation_error: &IntCounter,
    ) -> Result<CanisterId, CanisterManagerError> {
        let canister_id = state.metadata.generate_new_canister_id().map_err(|err| {
            error!(self.log, "Unable to generate new canister IDs: {}", err);
            CanisterManagerError::SubnetOutOfCanisterIds
        })?;

        // Sanity check: ensure that no canister with this ID exists already.
        debug_assert!(state.canister_state(&canister_id).is_none());
        if state.canister_state(&canister_id).is_some() {
            canister_creation_error.inc();
            error!(
                self.log,
                "[EXC-BUG] New canister id {} already exists.", canister_id
            );
            return Err(CanisterManagerError::CanisterIdAlreadyExists(canister_id));
        }

        Ok(canister_id)
    }

    fn validate_canister_exists<'a>(
        &self,
        state: &'a ReplicatedState,
        canister_id: CanisterId,
    ) -> Result<&'a CanisterState, CanisterManagerError> {
        state
            .canister_state(&canister_id)
            .ok_or(CanisterManagerError::CanisterNotFound(canister_id))
    }

    pub(crate) fn upload_chunk(
        &self,
        sender: PrincipalId,
        canister: &mut CanisterState,
        chunk: &[u8],
        round_limits: &mut RoundLimits,
        subnet_size: usize,
        resource_saturation: &ResourceSaturation,
    ) -> Result<UploadChunkResult, CanisterManagerError> {
        // Allow the canister itself to perform this operation.
        if sender != canister.system_state.canister_id.into() {
            validate_controller(canister, &sender)?
        }

        canister
            .system_state
            .wasm_chunk_store
            .can_insert_chunk(self.config.wasm_chunk_store_max_size, chunk)
            .map_err(|err| CanisterManagerError::WasmChunkStoreError { message: err })?;

        let chunk_bytes = wasm_chunk_store::chunk_size();
        let new_memory_usage = canister.memory_usage() + chunk_bytes;
        let instructions = self.config.upload_wasm_chunk_instructions;

        if self.config.rate_limiting_of_heap_delta == FlagStatus::Enabled
            && canister.scheduler_state.heap_delta_debit >= self.config.heap_delta_rate_limit
        {
            return Err(CanisterManagerError::WasmChunkStoreError {
                message: format!(
                    "Canister is heap delta rate limited. Current delta debit: {}, limit: {}",
                    canister.scheduler_state.heap_delta_debit, self.config.heap_delta_rate_limit
                ),
            });
        }

        let memory_usage = canister.memory_usage();
        let message_memory_usage = canister.message_memory_usage();
        let compute_allocation = canister.compute_allocation();
        let reveal_top_up = canister.controllers().contains(&sender);

        // Charge for the upload.
        let prepaid_cycles = self
            .cycles_account_manager
            .prepay_execution_cycles(
                &mut canister.system_state,
                memory_usage,
                message_memory_usage,
                compute_allocation,
                instructions,
                subnet_size,
                reveal_top_up,
                // For the upload chunk operation, it does not matter if this is a Wasm64 or Wasm32 module
                // since the number of instructions charged is a constant set fee and Wasm64 does not bring
                // any additional overhead for this operation. The only overhead is during execution time.
                WasmExecutionMode::Wasm32,
            )
            .map_err(|err| CanisterManagerError::WasmChunkStoreError {
                message: format!("Error charging for 'upload_chunk': {}", err),
            })?;
        // To keep the invariant that `prepay_execution_cycles` is always paired
        // with `refund_unused_execution_cycles` we refund zero immediately.
        self.cycles_account_manager.refund_unused_execution_cycles(
            &mut canister.system_state,
            NumInstructions::from(0),
            instructions,
            prepaid_cycles,
            // This counter is incremented if we refund more
            // instructions than initially charged, which is impossible
            // here.
            &IntCounter::new("no_op", "no_op").unwrap(),
            subnet_size,
            WasmExecutionMode::Wasm32,
            &self.log,
        );

        match canister.memory_allocation() {
            MemoryAllocation::Reserved(bytes) => {
                if bytes < new_memory_usage {
                    return Err(CanisterManagerError::NotEnoughMemoryAllocationGiven {
                        memory_allocation_given: canister.memory_allocation(),
                        memory_usage_needed: new_memory_usage,
                    });
                }
            }
            MemoryAllocation::BestEffort => {
                // Run the following checks on memory usage and return an error
                // if any fails:
                // 1. Check new usage will not freeze canister
                // 2. Check subnet has available memory
                // 3. Reserve cycles on canister
                // 4. Actually deduct memory from subnet (asserting it won't fail)

                // Calculate if any cycles will need to be reserved.
                let reservation_cycles = self.cycles_account_manager.storage_reservation_cycles(
                    chunk_bytes,
                    resource_saturation,
                    subnet_size,
                );

                // Memory usage will increase by the chunk size, so we need to
                // check that it doesn't bump us over the freezing threshold.
                let threshold = self.cycles_account_manager.freeze_threshold_cycles(
                    canister.system_state.freeze_threshold,
                    canister.memory_allocation(),
                    new_memory_usage,
                    canister.message_memory_usage(),
                    canister.compute_allocation(),
                    subnet_size,
                    canister.system_state.reserved_balance() + reservation_cycles,
                );
                // Note: if the subtraction here saturates, then we will get an
                // error later when trying to actually reserve the cycles.
                if threshold > canister.system_state.balance() - reservation_cycles {
                    return Err(CanisterManagerError::WasmChunkStoreError {
                        message: format!(
                            "Cannot upload chunk. At least {} additional cycles are required.",
                            threshold - canister.system_state.balance()
                        ),
                    });
                }
                // Verify subnet has enough memory.
                round_limits
                    .subnet_available_memory
                    .check_available_memory(chunk_bytes, NumBytes::from(0), NumBytes::from(0))
                    .map_err(
                        |_| CanisterManagerError::SubnetMemoryCapacityOverSubscribed {
                            requested: chunk_bytes,
                            available: NumBytes::from(
                                round_limits
                                    .subnet_available_memory
                                    .get_execution_memory()
                                    .max(0) as u64,
                            ),
                        },
                    )?;
                // Reserve needed cycles if the subnet is becoming saturated.
                canister
                    .system_state
                    .reserve_cycles(reservation_cycles)
                    .map_err(|err| match err {
                        ReservationError::InsufficientCycles {
                            requested,
                            available,
                        } => CanisterManagerError::InsufficientCyclesInMemoryGrow {
                            bytes: chunk_bytes,
                            available,
                            required: requested,
                        },
                        ReservationError::ReservedLimitExceed { requested, limit } => {
                            CanisterManagerError::ReservedCyclesLimitExceededInMemoryGrow {
                                bytes: chunk_bytes,
                                requested,
                                limit,
                            }
                        }
                    })?;
                // Actually deduct memory from the subnet. It's safe to unwrap
                // here because we already checked the available memory above.
                round_limits.subnet_available_memory
                            .try_decrement(chunk_bytes, NumBytes::from(0), NumBytes::from(0))
                            .expect("Error: Cannot fail to decrement SubnetAvailableMemory after checking for availability");
            }
        };

        if self.config.rate_limiting_of_heap_delta == FlagStatus::Enabled {
            canister.scheduler_state.heap_delta_debit += chunk_bytes;
        }

        round_limits.instructions -= as_round_instructions(instructions);

        // We initially checked that this chunk can be inserted, so the unwarp
        // here is guaranteed to succeed.
        let hash = canister
            .system_state
            .wasm_chunk_store
            .insert_chunk(self.config.wasm_chunk_store_max_size, chunk)
            .expect("Error: Insert chunk cannot fail after checking `can_insert_chunk`");
        Ok(UploadChunkResult {
            reply: UploadChunkReply {
                hash: hash.to_vec(),
            },
            heap_delta_increase: chunk_bytes,
        })
    }

    pub(crate) fn clear_chunk_store(
        &self,
        sender: PrincipalId,
        canister: &mut CanisterState,
    ) -> Result<(), CanisterManagerError> {
        // Allow the canister itself to perform this operation.
        if sender != canister.system_state.canister_id.into() {
            validate_controller(canister, &sender)?
        }
        canister.system_state.wasm_chunk_store = WasmChunkStore::new(Arc::clone(&self.fd_factory));
        Ok(())
    }

    pub(crate) fn stored_chunks(
        &self,
        sender: PrincipalId,
        canister: &CanisterState,
    ) -> Result<StoredChunksReply, CanisterManagerError> {
        // Allow the canister itself to perform this operation.
        if sender != canister.system_state.canister_id.into() {
            validate_controller(canister, &sender)?
        }

        let keys = canister
            .system_state
            .wasm_chunk_store
            .keys()
            .map(|k| ChunkHash { hash: k.to_vec() })
            .collect();
        Ok(StoredChunksReply(keys))
    }

    // Runs the following checks on memory usage and return an error
    // if any fails:
    // 1. Check new usage will not freeze canister
    // 2. Check subnet has available memory

    // Additionally calculates if any cycles will need to be reserved.
    //
    // This is to be used when taking or loading a canister snapshot
    // to ensure consistency in checks.
    fn memory_usage_checks(
        &self,
        subnet_size: usize,
        canister: &mut CanisterState,
        round_limits: &RoundLimits,
        new_memory_usage: NumBytes,
        old_memory_usage: NumBytes,
        resource_saturation: &ResourceSaturation,
    ) -> Result<(), CanisterManagerError> {
        let memory_increase = new_memory_usage.saturating_sub(&old_memory_usage);

        let reservation_cycles = self.cycles_account_manager.storage_reservation_cycles(
            memory_increase,
            resource_saturation,
            subnet_size,
        );

        // Check that the canister does not exceed its freezing threshold based
        // on the new memory usage.
        let threshold = self.cycles_account_manager.freeze_threshold_cycles(
            canister.system_state.freeze_threshold,
            canister.memory_allocation(),
            new_memory_usage,
            canister.message_memory_usage(),
            canister.compute_allocation(),
            subnet_size,
            canister.system_state.reserved_balance(),
        );

        if canister.system_state.balance() < threshold + reservation_cycles {
            return Err(CanisterManagerError::InsufficientCyclesInMemoryGrow {
                bytes: memory_increase,
                available: canister.system_state.balance(),
                required: threshold + reservation_cycles,
            });
        }

        // Verify that the subnet has enough memory available to satisfy the
        // requested change by the canister.
        round_limits
            .subnet_available_memory
            .check_available_memory(memory_increase, NumBytes::from(0), NumBytes::from(0))
            .map_err(
                |_| CanisterManagerError::SubnetMemoryCapacityOverSubscribed {
                    requested: memory_increase,
                    available: NumBytes::from(
                        round_limits
                            .subnet_available_memory
                            .get_execution_memory()
                            .max(0) as u64,
                    ),
                },
            )?;

        // Reserve needed cycles if the subnet is becoming saturated.
        canister
            .system_state
            .reserve_cycles(reservation_cycles)
            .map_err(|err| match err {
                ReservationError::InsufficientCycles {
                    requested,
                    available,
                } => CanisterManagerError::InsufficientCyclesInMemoryGrow {
                    bytes: memory_increase,
                    available,
                    required: requested,
                },
                ReservationError::ReservedLimitExceed { requested, limit } => {
                    CanisterManagerError::ReservedCyclesLimitExceededInMemoryGrow {
                        bytes: memory_increase,
                        requested,
                        limit,
                    }
                }
            })?;

        Ok(())
    }

    /// Creates a new canister snapshot.
    ///
    /// A canister snapshot can only be initiated by the controllers.
    /// In addition, if the `replace_snapshot` parameter is `Some`,
    /// the system will attempt to identify the snapshot based on the provided ID,
    /// and delete it before creating a new one.
    /// Failure to do so will result in the creation of a new snapshot being unsuccessful.
    ///
    /// If the new snapshot cannot be created, an appropriate error will be returned.
    pub(crate) fn take_canister_snapshot(
        &self,
        subnet_size: usize,
        sender: PrincipalId,
        canister: &mut CanisterState,
        replace_snapshot: Option<SnapshotId>,
        state: &mut ReplicatedState,
        round_limits: &mut RoundLimits,
        resource_saturation: &ResourceSaturation,
    ) -> (
        Result<CanisterSnapshotResponse, CanisterManagerError>,
        NumInstructions,
    ) {
        // Check sender is a controller.
        if let Err(err) = validate_controller(canister, &sender) {
            return (Err(err), NumInstructions::new(0));
        };

        let replace_snapshot_size = match replace_snapshot {
            // Check that replace snapshot ID exists if provided.
            Some(replace_snapshot) => {
                match state.canister_snapshots.get(replace_snapshot) {
                    None => {
                        // If not found, the operation fails due to invalid parameters.
                        return (
                            Err(CanisterManagerError::CanisterSnapshotNotFound {
                                canister_id: canister.canister_id(),
                                snapshot_id: replace_snapshot,
                            }),
                            NumInstructions::new(0),
                        );
                    }
                    Some(snapshot) => {
                        // Verify the provided replacement snapshot belongs to this canister.
                        if snapshot.canister_id() != canister.canister_id() {
                            return (
                                Err(CanisterManagerError::CanisterSnapshotInvalidOwnership {
                                    canister_id: canister.canister_id(),
                                    snapshot_id: replace_snapshot,
                                }),
                                NumInstructions::new(0),
                            );
                        }
                        snapshot.size()
                    }
                }
            }
            // No replace snapshot ID provided, check whether the maximum number of snapshots
            // has been reached.
            None => {
                if state
                    .canister_snapshots
                    .count_by_canister(&canister.canister_id())
                    >= self.config.max_number_of_snapshots_per_canister
                {
                    return (
                        Err(CanisterManagerError::CanisterSnapshotLimitExceeded {
                            canister_id: canister.canister_id(),
                            limit: self.config.max_number_of_snapshots_per_canister,
                        }),
                        NumInstructions::new(0),
                    );
                }
                0.into()
            }
        };

        if self.config.rate_limiting_of_heap_delta == FlagStatus::Enabled
            && canister.scheduler_state.heap_delta_debit >= self.config.heap_delta_rate_limit
        {
            return (
                Err(CanisterManagerError::CanisterHeapDeltaRateLimited {
                    canister_id: canister.canister_id(),
                    value: canister.scheduler_state.heap_delta_debit,
                    limit: self.config.heap_delta_rate_limit,
                }),
                NumInstructions::new(0),
            );
        }

        let new_snapshot_size = canister.snapshot_size_bytes();

        let old_memory_usage = canister.memory_usage();
        let new_memory_usage = canister
            .memory_usage()
            .saturating_add(&new_snapshot_size)
            .saturating_sub(&replace_snapshot_size);
        if let Err(err) = self.memory_usage_checks(
            subnet_size,
            canister,
            round_limits,
            new_memory_usage,
            old_memory_usage,
            resource_saturation,
        ) {
            return (Err(err), NumInstructions::from(0));
        }

        // Charge for taking a snapshot of the canister.
        let instructions = self
            .config
            .canister_snapshot_baseline_instructions
            .saturating_add(&new_snapshot_size.get().into());

        if let Err(err) = self.cycles_account_manager.consume_cycles_for_instructions(
            &sender,
            canister,
            instructions,
            subnet_size,
            // For the `take_canister_snapshot` operation, it does not matter if this is a Wasm64 or Wasm32 module
            // since the number of instructions charged depends on constant set fee and snapshot size
            // and Wasm64 does not bring any additional overhead for this operation.
            // The only overhead is during execution time.
            WasmExecutionMode::Wasm32,
        ) {
            return (
                Err(CanisterManagerError::CanisterSnapshotNotEnoughCycles(err)),
                0.into(),
            );
        };

        // Create new snapshot.
        let new_snapshot = match CanisterSnapshot::from_canister(canister, state.time())
            .map_err(CanisterManagerError::from)
        {
            Ok(s) => s,
            Err(err) => return (Err(err), instructions),
        };

        // Delete old snapshot identified by `replace_snapshot` ID.
        if let Some(replace_snapshot) = replace_snapshot {
            state.canister_snapshots.remove(replace_snapshot);
            canister.system_state.snapshots_memory_usage = canister
                .system_state
                .snapshots_memory_usage
                .get()
                .saturating_sub(replace_snapshot_size.get())
                .into();
            // Confirm that `snapshots_memory_usage` is updated correctly.
            debug_assert_eq!(
                canister.system_state.snapshots_memory_usage,
                state
                    .canister_snapshots
                    .compute_memory_usage_by_canister(canister.canister_id()),
            );
            round_limits.subnet_available_memory.increment(
                replace_snapshot_size,
                NumBytes::from(0),
                NumBytes::from(0),
            );
        }

        // Actually deduct memory from the subnet. It's safe to unwrap
        // here because we already checked the available memory above.
        round_limits.subnet_available_memory
            .try_decrement(new_snapshot_size, NumBytes::from(0), NumBytes::from(0))
            .expect("Error: Cannot fail to decrement SubnetAvailableMemory after checking for availability");

        if self.config.rate_limiting_of_heap_delta == FlagStatus::Enabled {
            canister.scheduler_state.heap_delta_debit = canister
                .scheduler_state
                .heap_delta_debit
                .saturating_add(&new_snapshot.heap_delta());
        }
        state.metadata.heap_delta_estimate = state
            .metadata
            .heap_delta_estimate
            .saturating_add(&new_snapshot.heap_delta());

        let snapshot_id =
            SnapshotId::from((canister.canister_id(), canister.new_local_snapshot_id()));
        state
            .canister_snapshots
            .push(snapshot_id, Arc::new(new_snapshot));
        canister.system_state.snapshots_memory_usage = canister
            .system_state
            .snapshots_memory_usage
            .saturating_add(&new_snapshot_size);
        (
            Ok(CanisterSnapshotResponse::new(
                &snapshot_id,
                state.time().as_nanos_since_unix_epoch(),
                new_snapshot_size,
            )),
            instructions,
        )
    }

    pub(crate) fn load_canister_snapshot(
        &self,
        subnet_size: usize,
        sender: PrincipalId,
        canister: &mut CanisterState,
        snapshot_id: SnapshotId,
        state: &mut ReplicatedState,
        round_limits: &mut RoundLimits,
        origin: CanisterChangeOrigin,
        resource_saturation: &ResourceSaturation,
        long_execution_already_in_progress: &IntCounter,
    ) -> (Result<CanisterState, CanisterManagerError>, NumInstructions) {
        let canister_id = canister.canister_id();
        // Check sender is a controller.
        if let Err(err) = validate_controller(canister, &sender) {
            return (Err(err), NumInstructions::new(0));
        }

        if self.config.rate_limiting_of_heap_delta == FlagStatus::Enabled
            && canister.scheduler_state.heap_delta_debit >= self.config.heap_delta_rate_limit
        {
            return (
                Err(CanisterManagerError::CanisterHeapDeltaRateLimited {
                    canister_id,
                    value: canister.scheduler_state.heap_delta_debit,
                    limit: self.config.heap_delta_rate_limit,
                }),
                NumInstructions::new(0),
            );
        }

        // Check that snapshot ID exists.
        let snapshot: &Arc<CanisterSnapshot> = match state.canister_snapshots.get(snapshot_id) {
            None => {
                // If not found, the operation fails due to invalid parameters.
                return (
                    Err(CanisterManagerError::CanisterSnapshotNotFound {
                        canister_id,
                        snapshot_id,
                    }),
                    NumInstructions::new(0),
                );
            }
            Some(snapshot) => {
                // Verify the provided snapshot id belongs to this canister.
                if snapshot.canister_id() != canister_id {
                    return (
                        Err(CanisterManagerError::CanisterSnapshotInvalidOwnership {
                            canister_id,
                            snapshot_id,
                        }),
                        NumInstructions::new(0),
                    );
                }
                snapshot
            }
        };

        // Check the precondition:
        // Unable to start executing a `load_canister_snapshot`
        // if there is already a long-running message in progress for the specified canister.
        match canister.next_execution() {
            NextExecution::None | NextExecution::StartNew => {}
            NextExecution::ContinueLong | NextExecution::ContinueInstallCode => {
                long_execution_already_in_progress.inc();
                error!(
                    self.log,
                    "[EXC-BUG] Attempted to start a new `load_canister_snapshot` execution while the previous execution is still in progress for {}.", canister_id
                );
                return (
                    Err(CanisterManagerError::LongExecutionAlreadyInProgress { canister_id }),
                    NumInstructions::new(0),
                );
            }
        }

        // All basic checks have passed, charge baseline instructions.
        let old_memory_usage = canister.memory_usage();
        let mut canister_clone = canister.clone();

        if let Err(err) = self.cycles_account_manager.consume_cycles_for_instructions(
            &sender,
            &mut canister_clone,
            self.config.canister_snapshot_baseline_instructions,
            subnet_size,
            // For the `load_canister_snapshot` operation, it does not matter if this is a Wasm64 or Wasm32 module
            // since the number of instructions charged depends on constant set fee
            // and Wasm64 does not bring any additional overhead for this operation.
            // The only overhead is during execution time.
            WasmExecutionMode::Wasm32,
        ) {
            return (
                Err(CanisterManagerError::CanisterSnapshotNotEnoughCycles(err)),
                0.into(),
            );
        };

        let (_old_execution_state, mut system_state, scheduler_state) = canister_clone.into_parts();

        let (instructions_used, new_execution_state) = {
            let execution_snapshot = snapshot.execution_snapshot();
            let new_wasm_hash = WasmHash::from(&execution_snapshot.wasm_binary);
            let compilation_cost_handling = if state
                .metadata
                .expected_compiled_wasms
                .contains(&new_wasm_hash)
            {
                CompilationCostHandling::CountReducedAmount
            } else {
                CompilationCostHandling::CountFullAmount
            };

            let (instructions_used, new_execution_state) = self.hypervisor.create_execution_state(
                execution_snapshot.wasm_binary.clone(),
                "NOT_USED".into(),
                canister_id,
                round_limits,
                compilation_cost_handling,
            );

            let mut new_execution_state = match new_execution_state {
                Ok(execution_state) => execution_state,
                Err(err) => {
                    let err = CanisterManagerError::from((canister_id, err));
                    return (Err(err), instructions_used);
                }
            };

            new_execution_state.exported_globals = execution_snapshot.exported_globals.clone();
            new_execution_state.stable_memory = Memory::from(&execution_snapshot.stable_memory);
            new_execution_state.wasm_memory = Memory::from(&execution_snapshot.wasm_memory);
            (instructions_used, Some(new_execution_state))
        };

        system_state.wasm_chunk_store = snapshot.chunk_store().clone();
        system_state
            .certified_data
            .clone_from(snapshot.certified_data());
        let wasm_execution_mode = new_execution_state
            .as_ref()
            .map_or(WasmExecutionMode::Wasm32, |exec_state| {
                exec_state.wasm_execution_mode
            });

        let mut new_canister =
            CanisterState::new(system_state, new_execution_state, scheduler_state);
        let new_memory_usage = new_canister.memory_usage();

        let memory_allocation_given =
            canister.memory_limit(self.get_max_canister_memory_size(wasm_execution_mode));

        if new_memory_usage > memory_allocation_given {
            return (
                Err(CanisterManagerError::NotEnoughMemoryAllocationGiven {
                    memory_allocation_given: canister.memory_allocation(),
                    memory_usage_needed: new_memory_usage,
                }),
                instructions_used,
            );
        }

        if let Err(err) = self.memory_usage_checks(
            subnet_size,
            canister,
            round_limits,
            new_memory_usage,
            old_memory_usage,
            resource_saturation,
        ) {
            return (Err(err), instructions_used);
        }

        // Actually deduct memory from the subnet. It's safe to unwrap
        // here because we already checked the available memory above.
        round_limits.subnet_available_memory.try_decrement(
            new_memory_usage.saturating_sub(&old_memory_usage),
            NumBytes::from(0),
            NumBytes::from(0),
        ).expect("Error: Cannot fail to decrement SubnetAvailableMemory after checking for availability");

        // Charge for loading the snapshot of the canister.
        if let Err(err) = self.cycles_account_manager.consume_cycles_for_instructions(
            &sender,
            &mut new_canister,
            instructions_used.saturating_add(&snapshot.size().get().into()),
            subnet_size,
            // In this case, when the canister is actually created from the snapshot, we need to check
            // if the canister is in wasm64 mode to account for its instruction usage.
            wasm_execution_mode,
        ) {
            return (
                Err(CanisterManagerError::CanisterSnapshotNotEnoughCycles(err)),
                instructions_used,
            );
        };

        // Increment canister version.
        new_canister.system_state.canister_version += 1;
        new_canister.system_state.add_canister_change(
            state.time(),
            origin,
            CanisterChangeDetails::load_snapshot(
                snapshot.canister_version(),
                snapshot_id.to_vec(),
                snapshot.taken_at_timestamp().as_nanos_since_unix_epoch(),
            ),
        );
        state
            .canister_snapshots
            .add_restore_operation(canister_id, snapshot_id);

        if self.config.rate_limiting_of_heap_delta == FlagStatus::Enabled {
            new_canister.scheduler_state.heap_delta_debit = new_canister
                .scheduler_state
                .heap_delta_debit
                .saturating_add(&new_canister.heap_delta());
        }
        state.metadata.heap_delta_estimate = state
            .metadata
            .heap_delta_estimate
            .saturating_add(&new_canister.heap_delta());

        (Ok(new_canister), instructions_used)
    }

    /// Returns the canister snapshots list, or
    /// an error if it failed to retrieve the information.
    ///
    /// Retrieving the canister snapshots list can only be initiated by the controllers.
    pub(crate) fn list_canister_snapshot(
        &self,
        sender: PrincipalId,
        canister: &CanisterState,
        state: &ReplicatedState,
    ) -> Result<Vec<CanisterSnapshotResponse>, CanisterManagerError> {
        // Check sender is a controller.
        validate_controller(canister, &sender)?;

        let mut responses = vec![];
        for (snapshot_id, snapshot) in state
            .canister_snapshots
            .list_snapshots(canister.canister_id())
        {
            let snapshot_response = CanisterSnapshotResponse::new(
                &snapshot_id,
                snapshot.taken_at_timestamp().as_nanos_since_unix_epoch(),
                snapshot.size(),
            );
            responses.push(snapshot_response);
        }

        Ok(responses)
    }

    /// Deletes the specified canister snapshot if it exists,
    /// or returns an error if it failed.
    ///
    /// Deleting a canister snapshot can only be initiated by the controllers.
    pub(crate) fn delete_canister_snapshot(
        &self,
        sender: PrincipalId,
        canister: &mut CanisterState,
        delete_snapshot_id: SnapshotId,
        state: &mut ReplicatedState,
        round_limits: &mut RoundLimits,
    ) -> Result<(), CanisterManagerError> {
        // Check sender is a controller.
        validate_controller(canister, &sender)?;

        match state.canister_snapshots.get(delete_snapshot_id) {
            None => {
                // If not found, the operation fails due to invalid parameters.
                return Err(CanisterManagerError::CanisterSnapshotNotFound {
                    canister_id: canister.canister_id(),
                    snapshot_id: delete_snapshot_id,
                });
            }
            Some(delete_snapshot) => {
                // Verify the provided `delete_snapshot_id` belongs to this canister.
                if delete_snapshot.canister_id() != canister.canister_id() {
                    return Err(CanisterManagerError::CanisterSnapshotInvalidOwnership {
                        canister_id: canister.canister_id(),
                        snapshot_id: delete_snapshot_id,
                    });
                }
            }
        }
        let old_snapshot = state.canister_snapshots.remove(delete_snapshot_id);
        // Already confirmed that `old_snapshot` exists.
        let old_snapshot_size = old_snapshot.unwrap().size();
        canister.system_state.snapshots_memory_usage = canister
            .system_state
            .snapshots_memory_usage
            .get()
            .saturating_sub(old_snapshot_size.get())
            .into();
        // Confirm that `snapshots_memory_usage` is updated correctly.
        debug_assert_eq!(
            canister.system_state.snapshots_memory_usage,
            state
                .canister_snapshots
                .compute_memory_usage_by_canister(canister.canister_id()),
        );
        round_limits.subnet_available_memory.increment(
            old_snapshot_size,
            NumBytes::from(0),
            NumBytes::from(0),
        );
        Ok(())
    }

    /// Depending on the canister architecture (Wasm32 or Wasm64), returns the
    /// maximum memory size that can be allocated by a canister.
    pub(crate) fn get_max_canister_memory_size(
        &self,
        wasm_execution_mode: WasmExecutionMode,
    ) -> NumBytes {
        match wasm_execution_mode {
            WasmExecutionMode::Wasm32 => self.config.max_canister_memory_size_wasm32,
            WasmExecutionMode::Wasm64 => self.config.max_canister_memory_size_wasm64,
        }
    }

    pub(crate) fn read_snapshot_metadata(
        &self,
        sender: PrincipalId,
        snapshot_id: SnapshotId,
        canister: &CanisterState,
        state: &ReplicatedState,
    ) -> Result<ReadCanisterSnapshotMetadataResponse, CanisterManagerError> {
        // Check sender is a controller.
        validate_controller(canister, &sender)?;
        // If not found, the operation fails due to invalid parameters.
        let Some(snapshot) = state.canister_snapshots.get(snapshot_id) else {
            return Err(CanisterManagerError::CanisterSnapshotNotFound {
                canister_id: canister.canister_id(),
                snapshot_id,
            });
        };
        // Verify the provided `snapshot_id` belongs to this canister.
        if snapshot.canister_id() != canister.canister_id() {
            return Err(CanisterManagerError::CanisterSnapshotInvalidOwnership {
                canister_id: canister.canister_id(),
                snapshot_id,
            });
        }

        Ok(ReadCanisterSnapshotMetadataResponse {
            source: snapshot.source(),
            taken_at_timestamp: snapshot.taken_at_timestamp().as_nanos_since_unix_epoch(),
            wasm_module_size: snapshot.execution_snapshot().wasm_binary.len() as u64,
            exported_globals: snapshot.exported_globals().clone(),
            wasm_memory_size: snapshot.execution_snapshot().wasm_memory.size.get() as u64
                * WASM_PAGE_SIZE_IN_BYTES as u64,
            stable_memory_size: snapshot.execution_snapshot().stable_memory.size.get() as u64
                * WASM_PAGE_SIZE_IN_BYTES as u64,
            wasm_chunk_store: snapshot
                .chunk_store()
                .keys()
                .cloned()
                .map(|x| ChunkHash { hash: x.to_vec() })
                .collect(),
            canister_version: snapshot.canister_version(),
            certified_data: snapshot.certified_data().clone(),
            global_timer: snapshot
                .execution_snapshot()
                .global_timer
                .map(GlobalTimer::from),
            on_low_wasm_memory_hook_status: snapshot
                .execution_snapshot()
                .on_low_wasm_memory_hook_status,
        })
    }

    pub(crate) fn read_snapshot_data(
        &self,
        sender: PrincipalId,
        canister: &mut CanisterState,
        snapshot_id: SnapshotId,
        kind: CanisterSnapshotDataKind,
        state: &ReplicatedState,
        subnet_size: usize,
    ) -> Result<ReadCanisterSnapshotDataResponse, CanisterManagerError> {
        // Check sender is a controller.
        validate_controller(canister, &sender)?;
        let Some(snapshot) = state.canister_snapshots.get(snapshot_id) else {
            return Err(CanisterManagerError::CanisterSnapshotNotFound {
                canister_id: canister.canister_id(),
                snapshot_id,
            });
        };
        // Verify the provided `snapshot_id` belongs to this canister.
        if snapshot.canister_id() != canister.canister_id() {
            return Err(CanisterManagerError::CanisterSnapshotInvalidOwnership {
                canister_id: canister.canister_id(),
                snapshot_id,
            });
        }

        // Charge upfront for the baseline plus the maximum possible size of the returned slice or fail.
        let num_response_bytes = match &kind {
            CanisterSnapshotDataKind::WasmModule { size, .. } => *size,
            CanisterSnapshotDataKind::MainMemory { size, .. } => *size,
            CanisterSnapshotDataKind::StableMemory { size, .. } => *size,
            // In this case, we might overcharge. But the stored chunks are also charged fully even if they are smaller.
            CanisterSnapshotDataKind::WasmChunk { .. } => CHUNK_SIZE,
        };
        let size = NumInstructions::new(num_response_bytes);
        if let Err(err) = self.cycles_account_manager.consume_cycles_for_instructions(
            &sender,
            canister,
            self.config
                .canister_snapshot_data_baseline_instructions
                .saturating_add(&size),
            subnet_size,
            // For the `read_snapshot_data` operation, it does not matter if this is a Wasm64 or Wasm32 module.
            WasmExecutionMode::Wasm32,
        ) {
            return Err(CanisterManagerError::CanisterSnapshotNotEnoughCycles(err));
        };

        let res = match kind {
            CanisterSnapshotDataKind::StableMemory { offset, size } => {
                if size > MAX_SLICE_SIZE_BYTES {
                    return Err(CanisterManagerError::InvalidSubslice { offset, size });
                }
                let stable_memory = snapshot.execution_snapshot().stable_memory.clone();
                match CanisterSnapshot::get_memory_chunk(stable_memory, offset, size) {
                    Ok(chunk) => Ok(chunk),
                    Err(e) => Err(e.into()),
                }
            }
            CanisterSnapshotDataKind::MainMemory { offset, size } => {
                if size > MAX_SLICE_SIZE_BYTES {
                    return Err(CanisterManagerError::InvalidSubslice { offset, size });
                }
                let main_memory = snapshot.execution_snapshot().wasm_memory.clone();
                match CanisterSnapshot::get_memory_chunk(main_memory, offset, size) {
                    Ok(chunk) => Ok(chunk),
                    Err(e) => Err(e.into()),
                }
            }
            CanisterSnapshotDataKind::WasmModule { offset, size } => {
                if size > MAX_SLICE_SIZE_BYTES {
                    return Err(CanisterManagerError::InvalidSubslice { offset, size });
                }
                match snapshot.get_wasm_module_chunk(offset, size) {
                    Ok(chunk) => Ok(chunk),
                    Err(e) => Err(e.into()),
                }
            }
            CanisterSnapshotDataKind::WasmChunk { hash } => {
                let Ok(hash) = <WasmChunkHash>::try_from(hash.clone()) else {
                    return Err(CanisterManagerError::WasmChunkStoreError {
                        message: format!("Bytes {:02x?} are not a valid WasmChunkHash.", hash),
                    });
                };
                let Some(chunk) = snapshot.chunk_store().get_chunk_complete(&hash) else {
                    return Err(CanisterManagerError::WasmChunkStoreError {
                        message: format!("WasmChunkHash {:02x?} not found.", hash),
                    });
                };
                Ok(chunk)
            }
        };
        res.map(ReadCanisterSnapshotDataResponse::new)
    }
}

/// Uninstalls a canister.
///
/// See https://internetcomputer.org/docs/current/references/ic-interface-spec#ic-uninstall_code
///
/// Returns a list of rejects that need to be sent out to their callers.
#[doc(hidden)]
pub fn uninstall_canister(
    log: &ReplicaLogger,
    canister: &mut CanisterState,
    time: Time,
    add_canister_change: AddCanisterChangeToHistory,
    fd_factory: Arc<dyn PageAllocatorFileDescriptor>,
) -> Vec<Response> {
    // Drop the canister's execution state.
    canister.execution_state = None;

    // Clear log.
    canister.clear_log();

    // Clear the Wasm chunk store.
    canister.system_state.wasm_chunk_store = WasmChunkStore::new(fd_factory);

    // Drop its certified data.
    canister.system_state.certified_data = Vec::new();

    // Deactivate global timer.
    canister.system_state.global_timer = CanisterTimer::Inactive;
    // Increment canister version.
    canister.system_state.canister_version += 1;
    match add_canister_change {
        AddCanisterChangeToHistory::Yes(origin) => {
            canister.system_state.add_canister_change(
                time,
                origin,
                CanisterChangeDetails::CanisterCodeUninstall,
            );
        }
        AddCanisterChangeToHistory::No => {}
    };

    let canister_id = canister.canister_id();
    let reject_responses = canister
        .system_state
        .delete_all_call_contexts(|call_context| {
            // Generate reject responses for ingress and canister messages.
            match call_context.call_origin() {
                CallOrigin::Ingress(user_id, message_id) => {
                    Some(Response::Ingress(IngressResponse {
                        message_id: message_id.clone(),
                        status: IngressStatus::Known {
                            receiver: canister_id.get(),
                            user_id: *user_id,
                            time,
                            state: IngressState::Failed(UserError::new(
                                ErrorCode::CanisterRejectedMessage,
                                "Canister has been uninstalled.",
                            )),
                        },
                    }))
                }
                CallOrigin::CanisterUpdate(caller_canister_id, callback_id, deadline) => {
                    Some(Response::Canister(CanisterResponse {
                        originator: *caller_canister_id,
                        respondent: canister_id,
                        originator_reply_callback: *callback_id,
                        refund: call_context.available_cycles(),
                        response_payload: Payload::Reject(RejectContext::new(
                            RejectCode::CanisterReject,
                            "Canister has been uninstalled.",
                        )),
                        deadline: *deadline,
                    }))
                }
                CallOrigin::CanisterQuery(_, _) | CallOrigin::Query(_) => fatal!(
                    log,
                    "No callbacks with a query origin should be found when uninstalling"
                ),
                CallOrigin::SystemTask => {
                    // Cannot respond to system tasks. Nothing to do.
                    None
                }
            }
        });

    reject_responses
}

#[cfg(test)]
pub(crate) mod tests;
