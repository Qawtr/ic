// This file is @generated by prost-build.
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct IngressStatusUnknown {}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IngressStatusProcessing {
    #[prost(message, optional, tag = "1")]
    pub user_id: ::core::option::Option<super::super::super::types::v1::UserId>,
    #[prost(uint64, tag = "2")]
    pub time_nanos: u64,
    #[prost(message, optional, tag = "3")]
    pub receiver: ::core::option::Option<super::super::super::types::v1::PrincipalId>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IngressStatusReceived {
    #[prost(message, optional, tag = "1")]
    pub user_id: ::core::option::Option<super::super::super::types::v1::UserId>,
    #[prost(uint64, tag = "2")]
    pub time_nanos: u64,
    #[prost(message, optional, tag = "3")]
    pub receiver: ::core::option::Option<super::super::super::types::v1::PrincipalId>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IngressStatusCompleted {
    #[prost(message, optional, tag = "1")]
    pub user_id: ::core::option::Option<super::super::super::types::v1::UserId>,
    #[prost(uint64, tag = "4")]
    pub time_nanos: u64,
    #[prost(message, optional, tag = "5")]
    pub receiver: ::core::option::Option<super::super::super::types::v1::PrincipalId>,
    #[prost(oneof = "ingress_status_completed::WasmResult", tags = "2, 3")]
    pub wasm_result: ::core::option::Option<ingress_status_completed::WasmResult>,
}
/// Nested message and enum types in `IngressStatusCompleted`.
pub mod ingress_status_completed {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum WasmResult {
        #[prost(bytes, tag = "2")]
        Reply(::prost::alloc::vec::Vec<u8>),
        #[prost(string, tag = "3")]
        Reject(::prost::alloc::string::String),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IngressStatusFailed {
    #[prost(message, optional, tag = "1")]
    pub user_id: ::core::option::Option<super::super::super::types::v1::UserId>,
    #[prost(string, tag = "3")]
    pub err_description: ::prost::alloc::string::String,
    #[prost(uint64, tag = "4")]
    pub time_nanos: u64,
    #[prost(message, optional, tag = "5")]
    pub receiver: ::core::option::Option<super::super::super::types::v1::PrincipalId>,
    #[prost(enumeration = "ErrorCode", tag = "6")]
    pub err_code: i32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IngressStatusDone {
    #[prost(message, optional, tag = "1")]
    pub user_id: ::core::option::Option<super::super::super::types::v1::UserId>,
    #[prost(uint64, tag = "2")]
    pub time_nanos: u64,
    #[prost(message, optional, tag = "3")]
    pub receiver: ::core::option::Option<super::super::super::types::v1::PrincipalId>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PruningEntry {
    #[prost(uint64, tag = "1")]
    pub time_nanos: u64,
    #[prost(bytes = "vec", repeated, tag = "2")]
    pub messages: ::prost::alloc::vec::Vec<::prost::alloc::vec::Vec<u8>>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IngressStatus {
    #[prost(oneof = "ingress_status::Status", tags = "1, 2, 3, 4, 5, 6")]
    pub status: ::core::option::Option<ingress_status::Status>,
}
/// Nested message and enum types in `IngressStatus`.
pub mod ingress_status {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Status {
        #[prost(message, tag = "1")]
        Unknown(super::IngressStatusUnknown),
        #[prost(message, tag = "2")]
        Processing(super::IngressStatusProcessing),
        #[prost(message, tag = "3")]
        Received(super::IngressStatusReceived),
        #[prost(message, tag = "4")]
        Completed(super::IngressStatusCompleted),
        #[prost(message, tag = "5")]
        Failed(super::IngressStatusFailed),
        #[prost(message, tag = "6")]
        Done(super::IngressStatusDone),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IngressStatusEntry {
    #[prost(bytes = "vec", tag = "1")]
    pub message_id: ::prost::alloc::vec::Vec<u8>,
    #[prost(message, optional, tag = "2")]
    pub status: ::core::option::Option<IngressStatus>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IngressHistoryState {
    #[prost(message, repeated, tag = "1")]
    pub statuses: ::prost::alloc::vec::Vec<IngressStatusEntry>,
    #[prost(message, repeated, tag = "2")]
    pub pruning_times: ::prost::alloc::vec::Vec<PruningEntry>,
    /// The earliest time in `pruning_times` with associated message IDs that
    /// may still be of type completed or failed.
    #[prost(uint64, tag = "3")]
    pub next_terminal_time: u64,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Ingress {
    #[prost(message, optional, tag = "1")]
    pub source: ::core::option::Option<super::super::super::types::v1::UserId>,
    #[prost(message, optional, tag = "2")]
    pub receiver: ::core::option::Option<super::super::super::types::v1::CanisterId>,
    #[prost(string, tag = "3")]
    pub method_name: ::prost::alloc::string::String,
    #[prost(bytes = "vec", tag = "4")]
    pub method_payload: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "5")]
    pub message_id: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "6")]
    pub expiry_time_nanos: u64,
    /// It may be present for a subnet message.
    /// Represents the id of the canister that the message is targeting.
    #[prost(message, optional, tag = "7")]
    pub effective_canister_id: ::core::option::Option<super::super::super::types::v1::CanisterId>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ErrorCode {
    Unspecified = 0,
    /// 1xx -- `RejectCode::SysFatal`
    SubnetOversubscribed = 101,
    MaxNumberOfCanistersReached = 102,
    /// 2xx -- `RejectCode::SysTransient`
    CanisterQueueFull = 201,
    IngressMessageTimeout = 202,
    CanisterQueueNotEmpty = 203,
    IngressHistoryFull = 204,
    CanisterIdAlreadyExists = 205,
    StopCanisterRequestTimeout = 206,
    CanisterOutOfCycles = 207,
    CertifiedStateUnavailable = 208,
    CanisterInstallCodeRateLimited = 209,
    CanisterHeapDeltaRateLimited = 210,
    /// 3xx -- `RejectCode::DestinationInvalid`
    CanisterNotFound = 301,
    CanisterSnapshotNotFound = 305,
    /// 4xx -- `RejectCode::CanisterReject`
    InsufficientMemoryAllocation = 402,
    InsufficientCyclesForCreateCanister = 403,
    SubnetNotFound = 404,
    CanisterNotHostedBySubnet = 405,
    CanisterRejectedMessage = 406,
    UnknownManagementMessage = 407,
    InvalidManagementPayload = 408,
    CanisterTrapped = 502,
    CanisterCalledTrap = 503,
    CanisterContractViolation = 504,
    CanisterInvalidWasm = 505,
    CanisterDidNotReply = 506,
    CanisterOutOfMemory = 507,
    CanisterStopped = 508,
    CanisterStopping = 509,
    CanisterNotStopped = 510,
    CanisterStoppingCancelled = 511,
    CanisterInvalidController = 512,
    CanisterFunctionNotFound = 513,
    CanisterNonEmpty = 514,
    QueryCallGraphLoopDetected = 517,
    InsufficientCyclesInCall = 520,
    CanisterWasmEngineError = 521,
    CanisterInstructionLimitExceeded = 522,
    CanisterMemoryAccessLimitExceeded = 524,
    QueryCallGraphTooDeep = 525,
    QueryCallGraphTotalInstructionLimitExceeded = 526,
    CompositeQueryCalledInReplicatedMode = 527,
    QueryTimeLimitExceeded = 528,
    QueryCallGraphInternal = 529,
    InsufficientCyclesInComputeAllocation = 530,
    InsufficientCyclesInMemoryAllocation = 531,
    InsufficientCyclesInMemoryGrow = 532,
    ReservedCyclesLimitExceededInMemoryAllocation = 533,
    ReservedCyclesLimitExceededInMemoryGrow = 534,
    InsufficientCyclesInMessageMemoryGrow = 535,
    CanisterMethodNotFound = 536,
    CanisterWasmModuleNotFound = 537,
    CanisterAlreadyInstalled = 538,
    CanisterWasmMemoryLimitExceeded = 539,
    ReservedCyclesLimitIsTooLow = 540,
    /// 6xx -- `RejectCode::SysUnknown`
    DeadlineExpired = 601,
    ResponseDropped = 602,
}
impl ErrorCode {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Unspecified => "ERROR_CODE_UNSPECIFIED",
            Self::SubnetOversubscribed => "ERROR_CODE_SUBNET_OVERSUBSCRIBED",
            Self::MaxNumberOfCanistersReached => "ERROR_CODE_MAX_NUMBER_OF_CANISTERS_REACHED",
            Self::CanisterQueueFull => "ERROR_CODE_CANISTER_QUEUE_FULL",
            Self::IngressMessageTimeout => "ERROR_CODE_INGRESS_MESSAGE_TIMEOUT",
            Self::CanisterQueueNotEmpty => "ERROR_CODE_CANISTER_QUEUE_NOT_EMPTY",
            Self::IngressHistoryFull => "ERROR_CODE_INGRESS_HISTORY_FULL",
            Self::CanisterIdAlreadyExists => "ERROR_CODE_CANISTER_ID_ALREADY_EXISTS",
            Self::StopCanisterRequestTimeout => "ERROR_CODE_STOP_CANISTER_REQUEST_TIMEOUT",
            Self::CanisterOutOfCycles => "ERROR_CODE_CANISTER_OUT_OF_CYCLES",
            Self::CertifiedStateUnavailable => "ERROR_CODE_CERTIFIED_STATE_UNAVAILABLE",
            Self::CanisterInstallCodeRateLimited => "ERROR_CODE_CANISTER_INSTALL_CODE_RATE_LIMITED",
            Self::CanisterHeapDeltaRateLimited => "ERROR_CODE_CANISTER_HEAP_DELTA_RATE_LIMITED",
            Self::CanisterNotFound => "ERROR_CODE_CANISTER_NOT_FOUND",
            Self::CanisterSnapshotNotFound => "ERROR_CODE_CANISTER_SNAPSHOT_NOT_FOUND",
            Self::InsufficientMemoryAllocation => "ERROR_CODE_INSUFFICIENT_MEMORY_ALLOCATION",
            Self::InsufficientCyclesForCreateCanister => {
                "ERROR_CODE_INSUFFICIENT_CYCLES_FOR_CREATE_CANISTER"
            }
            Self::SubnetNotFound => "ERROR_CODE_SUBNET_NOT_FOUND",
            Self::CanisterNotHostedBySubnet => "ERROR_CODE_CANISTER_NOT_HOSTED_BY_SUBNET",
            Self::CanisterRejectedMessage => "ERROR_CODE_CANISTER_REJECTED_MESSAGE",
            Self::UnknownManagementMessage => "ERROR_CODE_UNKNOWN_MANAGEMENT_MESSAGE",
            Self::InvalidManagementPayload => "ERROR_CODE_INVALID_MANAGEMENT_PAYLOAD",
            Self::CanisterTrapped => "ERROR_CODE_CANISTER_TRAPPED",
            Self::CanisterCalledTrap => "ERROR_CODE_CANISTER_CALLED_TRAP",
            Self::CanisterContractViolation => "ERROR_CODE_CANISTER_CONTRACT_VIOLATION",
            Self::CanisterInvalidWasm => "ERROR_CODE_CANISTER_INVALID_WASM",
            Self::CanisterDidNotReply => "ERROR_CODE_CANISTER_DID_NOT_REPLY",
            Self::CanisterOutOfMemory => "ERROR_CODE_CANISTER_OUT_OF_MEMORY",
            Self::CanisterStopped => "ERROR_CODE_CANISTER_STOPPED",
            Self::CanisterStopping => "ERROR_CODE_CANISTER_STOPPING",
            Self::CanisterNotStopped => "ERROR_CODE_CANISTER_NOT_STOPPED",
            Self::CanisterStoppingCancelled => "ERROR_CODE_CANISTER_STOPPING_CANCELLED",
            Self::CanisterInvalidController => "ERROR_CODE_CANISTER_INVALID_CONTROLLER",
            Self::CanisterFunctionNotFound => "ERROR_CODE_CANISTER_FUNCTION_NOT_FOUND",
            Self::CanisterNonEmpty => "ERROR_CODE_CANISTER_NON_EMPTY",
            Self::QueryCallGraphLoopDetected => "ERROR_CODE_QUERY_CALL_GRAPH_LOOP_DETECTED",
            Self::InsufficientCyclesInCall => "ERROR_CODE_INSUFFICIENT_CYCLES_IN_CALL",
            Self::CanisterWasmEngineError => "ERROR_CODE_CANISTER_WASM_ENGINE_ERROR",
            Self::CanisterInstructionLimitExceeded => {
                "ERROR_CODE_CANISTER_INSTRUCTION_LIMIT_EXCEEDED"
            }
            Self::CanisterMemoryAccessLimitExceeded => {
                "ERROR_CODE_CANISTER_MEMORY_ACCESS_LIMIT_EXCEEDED"
            }
            Self::QueryCallGraphTooDeep => "ERROR_CODE_QUERY_CALL_GRAPH_TOO_DEEP",
            Self::QueryCallGraphTotalInstructionLimitExceeded => {
                "ERROR_CODE_QUERY_CALL_GRAPH_TOTAL_INSTRUCTION_LIMIT_EXCEEDED"
            }
            Self::CompositeQueryCalledInReplicatedMode => {
                "ERROR_CODE_COMPOSITE_QUERY_CALLED_IN_REPLICATED_MODE"
            }
            Self::QueryTimeLimitExceeded => "ERROR_CODE_QUERY_TIME_LIMIT_EXCEEDED",
            Self::QueryCallGraphInternal => "ERROR_CODE_QUERY_CALL_GRAPH_INTERNAL",
            Self::InsufficientCyclesInComputeAllocation => {
                "ERROR_CODE_INSUFFICIENT_CYCLES_IN_COMPUTE_ALLOCATION"
            }
            Self::InsufficientCyclesInMemoryAllocation => {
                "ERROR_CODE_INSUFFICIENT_CYCLES_IN_MEMORY_ALLOCATION"
            }
            Self::InsufficientCyclesInMemoryGrow => "ERROR_CODE_INSUFFICIENT_CYCLES_IN_MEMORY_GROW",
            Self::ReservedCyclesLimitExceededInMemoryAllocation => {
                "ERROR_CODE_RESERVED_CYCLES_LIMIT_EXCEEDED_IN_MEMORY_ALLOCATION"
            }
            Self::ReservedCyclesLimitExceededInMemoryGrow => {
                "ERROR_CODE_RESERVED_CYCLES_LIMIT_EXCEEDED_IN_MEMORY_GROW"
            }
            Self::InsufficientCyclesInMessageMemoryGrow => {
                "ERROR_CODE_INSUFFICIENT_CYCLES_IN_MESSAGE_MEMORY_GROW"
            }
            Self::CanisterMethodNotFound => "ERROR_CODE_CANISTER_METHOD_NOT_FOUND",
            Self::CanisterWasmModuleNotFound => "ERROR_CODE_CANISTER_WASM_MODULE_NOT_FOUND",
            Self::CanisterAlreadyInstalled => "ERROR_CODE_CANISTER_ALREADY_INSTALLED",
            Self::CanisterWasmMemoryLimitExceeded => {
                "ERROR_CODE_CANISTER_WASM_MEMORY_LIMIT_EXCEEDED"
            }
            Self::ReservedCyclesLimitIsTooLow => "ERROR_CODE_RESERVED_CYCLES_LIMIT_IS_TOO_LOW",
            Self::DeadlineExpired => "ERROR_CODE_DEADLINE_EXPIRED",
            Self::ResponseDropped => "ERROR_CODE_RESPONSE_DROPPED",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "ERROR_CODE_UNSPECIFIED" => Some(Self::Unspecified),
            "ERROR_CODE_SUBNET_OVERSUBSCRIBED" => Some(Self::SubnetOversubscribed),
            "ERROR_CODE_MAX_NUMBER_OF_CANISTERS_REACHED" => Some(Self::MaxNumberOfCanistersReached),
            "ERROR_CODE_CANISTER_QUEUE_FULL" => Some(Self::CanisterQueueFull),
            "ERROR_CODE_INGRESS_MESSAGE_TIMEOUT" => Some(Self::IngressMessageTimeout),
            "ERROR_CODE_CANISTER_QUEUE_NOT_EMPTY" => Some(Self::CanisterQueueNotEmpty),
            "ERROR_CODE_INGRESS_HISTORY_FULL" => Some(Self::IngressHistoryFull),
            "ERROR_CODE_CANISTER_ID_ALREADY_EXISTS" => Some(Self::CanisterIdAlreadyExists),
            "ERROR_CODE_STOP_CANISTER_REQUEST_TIMEOUT" => Some(Self::StopCanisterRequestTimeout),
            "ERROR_CODE_CANISTER_OUT_OF_CYCLES" => Some(Self::CanisterOutOfCycles),
            "ERROR_CODE_CERTIFIED_STATE_UNAVAILABLE" => Some(Self::CertifiedStateUnavailable),
            "ERROR_CODE_CANISTER_INSTALL_CODE_RATE_LIMITED" => {
                Some(Self::CanisterInstallCodeRateLimited)
            }
            "ERROR_CODE_CANISTER_HEAP_DELTA_RATE_LIMITED" => {
                Some(Self::CanisterHeapDeltaRateLimited)
            }
            "ERROR_CODE_CANISTER_NOT_FOUND" => Some(Self::CanisterNotFound),
            "ERROR_CODE_CANISTER_SNAPSHOT_NOT_FOUND" => Some(Self::CanisterSnapshotNotFound),
            "ERROR_CODE_INSUFFICIENT_MEMORY_ALLOCATION" => Some(Self::InsufficientMemoryAllocation),
            "ERROR_CODE_INSUFFICIENT_CYCLES_FOR_CREATE_CANISTER" => {
                Some(Self::InsufficientCyclesForCreateCanister)
            }
            "ERROR_CODE_SUBNET_NOT_FOUND" => Some(Self::SubnetNotFound),
            "ERROR_CODE_CANISTER_NOT_HOSTED_BY_SUBNET" => Some(Self::CanisterNotHostedBySubnet),
            "ERROR_CODE_CANISTER_REJECTED_MESSAGE" => Some(Self::CanisterRejectedMessage),
            "ERROR_CODE_UNKNOWN_MANAGEMENT_MESSAGE" => Some(Self::UnknownManagementMessage),
            "ERROR_CODE_INVALID_MANAGEMENT_PAYLOAD" => Some(Self::InvalidManagementPayload),
            "ERROR_CODE_CANISTER_TRAPPED" => Some(Self::CanisterTrapped),
            "ERROR_CODE_CANISTER_CALLED_TRAP" => Some(Self::CanisterCalledTrap),
            "ERROR_CODE_CANISTER_CONTRACT_VIOLATION" => Some(Self::CanisterContractViolation),
            "ERROR_CODE_CANISTER_INVALID_WASM" => Some(Self::CanisterInvalidWasm),
            "ERROR_CODE_CANISTER_DID_NOT_REPLY" => Some(Self::CanisterDidNotReply),
            "ERROR_CODE_CANISTER_OUT_OF_MEMORY" => Some(Self::CanisterOutOfMemory),
            "ERROR_CODE_CANISTER_STOPPED" => Some(Self::CanisterStopped),
            "ERROR_CODE_CANISTER_STOPPING" => Some(Self::CanisterStopping),
            "ERROR_CODE_CANISTER_NOT_STOPPED" => Some(Self::CanisterNotStopped),
            "ERROR_CODE_CANISTER_STOPPING_CANCELLED" => Some(Self::CanisterStoppingCancelled),
            "ERROR_CODE_CANISTER_INVALID_CONTROLLER" => Some(Self::CanisterInvalidController),
            "ERROR_CODE_CANISTER_FUNCTION_NOT_FOUND" => Some(Self::CanisterFunctionNotFound),
            "ERROR_CODE_CANISTER_NON_EMPTY" => Some(Self::CanisterNonEmpty),
            "ERROR_CODE_QUERY_CALL_GRAPH_LOOP_DETECTED" => Some(Self::QueryCallGraphLoopDetected),
            "ERROR_CODE_INSUFFICIENT_CYCLES_IN_CALL" => Some(Self::InsufficientCyclesInCall),
            "ERROR_CODE_CANISTER_WASM_ENGINE_ERROR" => Some(Self::CanisterWasmEngineError),
            "ERROR_CODE_CANISTER_INSTRUCTION_LIMIT_EXCEEDED" => {
                Some(Self::CanisterInstructionLimitExceeded)
            }
            "ERROR_CODE_CANISTER_MEMORY_ACCESS_LIMIT_EXCEEDED" => {
                Some(Self::CanisterMemoryAccessLimitExceeded)
            }
            "ERROR_CODE_QUERY_CALL_GRAPH_TOO_DEEP" => Some(Self::QueryCallGraphTooDeep),
            "ERROR_CODE_QUERY_CALL_GRAPH_TOTAL_INSTRUCTION_LIMIT_EXCEEDED" => {
                Some(Self::QueryCallGraphTotalInstructionLimitExceeded)
            }
            "ERROR_CODE_COMPOSITE_QUERY_CALLED_IN_REPLICATED_MODE" => {
                Some(Self::CompositeQueryCalledInReplicatedMode)
            }
            "ERROR_CODE_QUERY_TIME_LIMIT_EXCEEDED" => Some(Self::QueryTimeLimitExceeded),
            "ERROR_CODE_QUERY_CALL_GRAPH_INTERNAL" => Some(Self::QueryCallGraphInternal),
            "ERROR_CODE_INSUFFICIENT_CYCLES_IN_COMPUTE_ALLOCATION" => {
                Some(Self::InsufficientCyclesInComputeAllocation)
            }
            "ERROR_CODE_INSUFFICIENT_CYCLES_IN_MEMORY_ALLOCATION" => {
                Some(Self::InsufficientCyclesInMemoryAllocation)
            }
            "ERROR_CODE_INSUFFICIENT_CYCLES_IN_MEMORY_GROW" => {
                Some(Self::InsufficientCyclesInMemoryGrow)
            }
            "ERROR_CODE_RESERVED_CYCLES_LIMIT_EXCEEDED_IN_MEMORY_ALLOCATION" => {
                Some(Self::ReservedCyclesLimitExceededInMemoryAllocation)
            }
            "ERROR_CODE_RESERVED_CYCLES_LIMIT_EXCEEDED_IN_MEMORY_GROW" => {
                Some(Self::ReservedCyclesLimitExceededInMemoryGrow)
            }
            "ERROR_CODE_INSUFFICIENT_CYCLES_IN_MESSAGE_MEMORY_GROW" => {
                Some(Self::InsufficientCyclesInMessageMemoryGrow)
            }
            "ERROR_CODE_CANISTER_METHOD_NOT_FOUND" => Some(Self::CanisterMethodNotFound),
            "ERROR_CODE_CANISTER_WASM_MODULE_NOT_FOUND" => Some(Self::CanisterWasmModuleNotFound),
            "ERROR_CODE_CANISTER_ALREADY_INSTALLED" => Some(Self::CanisterAlreadyInstalled),
            "ERROR_CODE_CANISTER_WASM_MEMORY_LIMIT_EXCEEDED" => {
                Some(Self::CanisterWasmMemoryLimitExceeded)
            }
            "ERROR_CODE_RESERVED_CYCLES_LIMIT_IS_TOO_LOW" => {
                Some(Self::ReservedCyclesLimitIsTooLow)
            }
            "ERROR_CODE_DEADLINE_EXPIRED" => Some(Self::DeadlineExpired),
            "ERROR_CODE_RESPONSE_DROPPED" => Some(Self::ResponseDropped),
            _ => None,
        }
    }
}
