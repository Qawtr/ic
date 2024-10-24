use ic_error_types::UserError;
use ic_management_canister_types::{self as ic00, CanisterSettingsArgs, Payload};
use ic_state_machine_tests::{StateMachine, StateMachineBuilder};
use ic_types::{
    ingress::{IngressState, IngressStatus, WasmResult},
    messages::MessageId,
    PrincipalId,
};
use std::time::Instant;

const MAX_TICKS: usize = 100;

fn get_result(status: IngressStatus) -> Option<Result<WasmResult, UserError>> {
    match status {
        IngressStatus::Known {
            state: IngressState::Completed(result),
            ..
        } => Some(Ok(result)),
        IngressStatus::Known {
            state: IngressState::Failed(error),
            ..
        } => Some(Err(error)),
        _ => None,
    }
}

fn await_ingress_responses(
    env: &StateMachine,
    message_ids: &[MessageId],
) -> Vec<Result<WasmResult, UserError>> {
    let start_time = Instant::now();

    for _ in 0..MAX_TICKS {
        let statuses: Vec<IngressStatus> = message_ids
            .iter()
            .map(|msg_id| env.ingress_status(msg_id))
            .collect();

        let results: Vec<_> = statuses.into_iter().filter_map(get_result).collect();
        if results.len() == message_ids.len() {
            return results;
        }

        env.tick();
    }

    panic!(
        "Ingress responses not received within {} ticks ({:?} elapsed)",
        MAX_TICKS,
        start_time.elapsed()
    );
}

fn main() {
    println!("Starting the canister creation process...");

    const CANISTERS_TO_CREATE: usize = 2;
    let env = StateMachineBuilder::default().build();
    let settings: Option<CanisterSettingsArgs> = None;

    let message_ids: Vec<MessageId> = (0..CANISTERS_TO_CREATE)
        .map(|_| {
            env.send_ingress(
                PrincipalId::new_anonymous(),
                ic00::IC_00,
                ic00::Method::ProvisionalCreateCanisterWithCycles,
                ic00::ProvisionalCreateCanisterWithCyclesArgs {
                    amount: Some((u128::MAX / 2).into()),
                    settings: settings.clone(),
                    specified_id: None,
                    sender_canister_version: None,
                }
                .encode(),
            )
        })
        .collect();

    let results = await_ingress_responses(&env, &message_ids);

    println!("Canister creation results: {:?}", results);
}
