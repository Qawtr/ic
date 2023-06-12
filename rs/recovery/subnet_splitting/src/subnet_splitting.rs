use crate::admin_helper::{
    get_halt_subnet_at_cup_height_command, get_propose_to_complete_canister_migration_command,
    get_propose_to_prepare_canister_migration_command,
    get_propose_to_reroute_canister_ranges_command,
};

use clap::Parser;
use ic_base_types::SubnetId;
use ic_recovery::{
    cli::read_optional,
    error::RecoveryResult,
    recovery_iterator::RecoveryIterator,
    recovery_state::{HasRecoveryState, RecoveryState},
    steps::{AdminStep, Step},
    NeuronArgs, Recovery, RecoveryArgs,
};
use ic_registry_routing_table::CanisterIdRange;
use serde::{Deserialize, Serialize};
use slog::Logger;
use std::{iter::Peekable, net::IpAddr};
use strum::{EnumMessage, IntoEnumIterator};
use strum_macros::{EnumIter, EnumString};

#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    EnumIter,
    EnumString,
    Serialize,
    Deserialize,
    EnumMessage,
    clap::ValueEnum,
)]
pub(crate) enum StepType {
    PrepareCanisterMigration,
    HaltSourceSubnetAtCupHeight,
    RerouteCanisterRanges,
    DownloadStateFromSourceSubnet,
    CopyDir,
    SplitOutSourceState,
    SplitOutDestinationState,
    ProposeCupForSourceSubnet,
    UploadStateToSourceSubnet,
    ProposeCupForDestinationSubnet,
    UploadStateToDestinationSubnet,
    WaitForCUPOnSourceSubnet,
    WaitForCUPOnDestinationSubnet,
    UnhaltSourceSubnet,
    UnhaltDestinationSubnet,
    CompleteCanisterMigration,
    Cleanup,
}

#[derive(Debug, Clone, PartialEq, Parser, Serialize, Deserialize)]
#[clap(version = "1.0")]
pub(crate) struct SubnetSplittingArgs {
    /// Id of the subnet whose state will be split.
    #[clap(long, parse(try_from_str=crate::util::subnet_id_from_str))]
    source_subnet_id: SubnetId,

    /// Id of the destination subnet.
    #[clap(long, parse(try_from_str=crate::util::subnet_id_from_str))]
    destination_subnet_id: SubnetId,

    /// Public ssh key to be deployed to the subnet for read only access.
    #[clap(long)]
    pub_key: Option<String>,

    /// If the downloaded state should be backed up locally.
    #[clap(long)]
    keep_downloaded_state: Option<bool>,

    /// IP address of the node from the source subnet to download the state from.
    #[clap(long)]
    download_node_source: Option<IpAddr>,

    /// IP address of the node to upload the new subnet state to.
    #[clap(long)]
    upload_node_source: Option<IpAddr>,

    /// IP address of the node to upload the new subnet state to.
    #[clap(long)]
    upload_node_destination: Option<IpAddr>,

    /// If present the tool will start execution for the provided step, skipping the initial ones.
    #[clap(long = "resume")]
    #[clap(value_enum)]
    next_step: Option<StepType>,

    /// The canister ID ranges to be moved to the destination subnet.
    #[clap(long, multiple_values(true), required = true)]
    canister_id_ranges_to_move: Vec<CanisterIdRange>,
}

pub(crate) struct SubnetSplitting {
    step_iterator: Peekable<StepTypeIter>,
    params: SubnetSplittingArgs,
    recovery_args: RecoveryArgs,
    neuron_args: Option<NeuronArgs>,
    recovery: Recovery,
    logger: Logger,
}

impl SubnetSplitting {
    pub(crate) fn new(
        logger: Logger,
        recovery_args: RecoveryArgs,
        neuron_args: Option<NeuronArgs>,
        subnet_args: SubnetSplittingArgs,
    ) -> Self {
        let recovery = Recovery::new(logger.clone(), recovery_args.clone(), neuron_args.clone())
            .expect("Failed to init recovery");
        recovery.init_registry_local_store();
        Self {
            step_iterator: StepType::iter().peekable(),
            params: subnet_args,
            recovery_args,
            neuron_args,
            recovery,
            logger,
        }
    }
}

impl RecoveryIterator<StepType, StepTypeIter> for SubnetSplitting {
    fn get_step_iterator(&mut self) -> &mut Peekable<StepTypeIter> {
        &mut self.step_iterator
    }

    fn store_next_step(&mut self, step_type: Option<StepType>) {
        self.params.next_step = step_type;
    }

    fn get_logger(&self) -> &Logger {
        &self.logger
    }

    fn interactive(&self) -> bool {
        true
    }

    fn read_step_params(&mut self, step_type: StepType) {
        match step_type {
            StepType::HaltSourceSubnetAtCupHeight => {
                if self.params.pub_key.is_none() {
                    self.params.pub_key = read_optional(
                        &self.logger,
                        "Enter public key to add readonly SSH access to subnet: ",
                    )
                }
            }

            StepType::DownloadStateFromSourceSubnet => todo!(),
            StepType::CopyDir => todo!(),
            StepType::SplitOutSourceState => todo!(),
            StepType::SplitOutDestinationState => todo!(),
            StepType::ProposeCupForSourceSubnet => todo!(),
            StepType::UploadStateToSourceSubnet => todo!(),
            StepType::ProposeCupForDestinationSubnet => todo!(),
            StepType::UploadStateToDestinationSubnet => todo!(),
            StepType::WaitForCUPOnSourceSubnet => todo!(),
            StepType::WaitForCUPOnDestinationSubnet => todo!(),
            StepType::UnhaltSourceSubnet => todo!(),
            StepType::UnhaltDestinationSubnet => todo!(),
            StepType::Cleanup => todo!(),
            _ => (),
        }
    }

    fn get_step_impl(&self, step_type: StepType) -> RecoveryResult<Box<dyn Step>> {
        let step: Box<dyn Step> = match step_type {
            StepType::PrepareCanisterMigration => AdminStep {
                logger: self.recovery.logger.clone(),
                ic_admin_cmd: get_propose_to_prepare_canister_migration_command(
                    &self.recovery.admin_helper,
                    &self.params.canister_id_ranges_to_move,
                    self.params.source_subnet_id,
                    self.params.destination_subnet_id,
                ),
            }
            .into(),

            StepType::HaltSourceSubnetAtCupHeight => AdminStep {
                logger: self.recovery.logger.clone(),
                ic_admin_cmd: get_halt_subnet_at_cup_height_command(
                    &self.recovery.admin_helper,
                    self.params.source_subnet_id,
                    &self.params.pub_key,
                ),
            }
            .into(),

            StepType::RerouteCanisterRanges => AdminStep {
                logger: self.recovery.logger.clone(),
                ic_admin_cmd: get_propose_to_reroute_canister_ranges_command(
                    &self.recovery.admin_helper,
                    &self.params.canister_id_ranges_to_move,
                    self.params.source_subnet_id,
                    self.params.destination_subnet_id,
                ),
            }
            .into(),

            StepType::DownloadStateFromSourceSubnet => todo!(),
            StepType::CopyDir => todo!(),
            StepType::SplitOutSourceState => todo!(),
            StepType::SplitOutDestinationState => todo!(),
            StepType::ProposeCupForSourceSubnet => todo!(),
            StepType::UploadStateToSourceSubnet => todo!(),
            StepType::ProposeCupForDestinationSubnet => todo!(),
            StepType::UploadStateToDestinationSubnet => todo!(),
            StepType::WaitForCUPOnSourceSubnet => todo!(),
            StepType::WaitForCUPOnDestinationSubnet => todo!(),
            StepType::UnhaltSourceSubnet => todo!(),
            StepType::UnhaltDestinationSubnet => todo!(),

            StepType::CompleteCanisterMigration => AdminStep {
                logger: self.recovery.logger.clone(),
                ic_admin_cmd: get_propose_to_complete_canister_migration_command(
                    &self.recovery.admin_helper,
                    &self.params.canister_id_ranges_to_move,
                    self.params.source_subnet_id,
                    self.params.destination_subnet_id,
                ),
            }
            .into(),

            StepType::Cleanup => todo!(),
        };

        Ok(step)
    }
}

impl Iterator for SubnetSplitting {
    type Item = (StepType, Box<dyn Step>);
    fn next(&mut self) -> Option<Self::Item> {
        self.next_step()
    }
}

impl HasRecoveryState for SubnetSplitting {
    type StepType = StepType;
    type SubcommandArgsType = SubnetSplittingArgs;

    fn get_next_step(&self) -> Option<Self::StepType> {
        self.params.next_step
    }

    fn get_state(&self) -> RecoveryResult<RecoveryState<Self::SubcommandArgsType>> {
        Ok(RecoveryState {
            recovery_args: self.recovery_args.clone(),
            neuron_args: self.neuron_args.clone(),
            subcommand_args: self.params.clone(),
        })
    }
}
