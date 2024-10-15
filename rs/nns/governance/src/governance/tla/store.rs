use local_key::task_local;
use std::sync::RwLock;
pub use tla_instrumentation::{InstrumentationState, UpdateTrace};

// The entire module should only ever be imported if the tla feature is enabled,
// but use another directive here just to make sure, as we really don't want to
// leak this into the production code.
#[cfg(feature = "tla")]
task_local! {
    pub static TLA_INSTRUMENTATION_STATE: InstrumentationState;
}

#[cfg(feature = "tla")]
pub static TLA_TRACES: RwLock<Vec<UpdateTrace>> = RwLock::new(Vec::new());
