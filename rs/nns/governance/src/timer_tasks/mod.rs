use ic_metrics_encoder::MetricsEncoder;
use ic_nervous_system_timer_task::{RecurringAsyncTask, TimerTaskMetricsRegistry};
use seeding::SeedingTask;
use std::cell::RefCell;

use crate::canister_state::GOVERNANCE;
use crate::timer_tasks::calculate_distributable_rewards::CalculateDistributableRewardsTask;

mod calculate_distributable_rewards;
mod distribute_rewards;
mod seeding;

thread_local! {
    static METRICS_REGISTRY: RefCell<TimerTaskMetricsRegistry> = RefCell::new(TimerTaskMetricsRegistry::default());
}

pub fn schedule_tasks() {
    SeedingTask::new(&GOVERNANCE).schedule(&METRICS_REGISTRY);
    CalculateDistributableRewardsTask::new(&GOVERNANCE).schedule(&METRICS_REGISTRY);
    run_distribute_rewards_periodic_task();
}

pub fn run_distribute_rewards_periodic_task() {
    distribute_rewards::run_distribute_rewards_periodic_task(&GOVERNANCE, &METRICS_REGISTRY);
}

/// Encodes the metrics for timer tasks.
pub fn encode_timer_task_metrics(encoder: &mut MetricsEncoder<Vec<u8>>) -> std::io::Result<()> {
    METRICS_REGISTRY.with(|registry| registry.borrow().encode("governance", encoder))
}
