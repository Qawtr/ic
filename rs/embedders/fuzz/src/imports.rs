// https://internetcomputer.org/docs/current/references/ic-interface-spec#system-api-imports
pub(crate) const SYSTEM_API_IMPORTS: &str = r#"
(module
  (type (;0;) (func))
  (type (;1;) (func (result i32)))
  (type (;2;) (func (result i64)))
  (type (;3;) (func (param i32)))
  (type (;4;) (func (param i32) (result i32)))
  (type (;5;) (func (param i32) (result i64)))
  (type (;6;) (func (param i32 i32)))
  (type (;7;) (func (param i32 i32) (result i32)))
  (type (;8;) (func (param i32 i32 i32)))
  (type (;9;) (func (param i32 i32 i32) (result i32)))
  (type (;10;) (func (param i32 i32 i32 i32)))
  (type (;11;) (func (param i32 i32 i32 i32) (result i32)))
  (type (;12;) (func (param i32 i32 i32 i32 i32)))
  (type (;13;) (func (param i32 i32 i32 i32 i32) (result i32)))
  (type (;14;) (func (param i32 i32 i32 i32 i32 i32)))
  (type (;15;) (func (param i32 i32 i32 i32 i32 i32) (result i32)))
  (type (;16;) (func (param i32 i32 i32 i32 i32 i32 i32) (result i32)))
  (type (;17;) (func (param i32 i32 i32 i32 i32 i32 i32 i32)))
  (type (;18;) (func (param i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32) (result i32)))
  (type (;19;) (func (param i32 i32 i64)))
  (type (;20;) (func (param i32 i32 i64 i64)))
  (type (;21;) (func (param i32 i64 i32 i32)))
  (type (;22;) (func (param i32 i64 i32 i32 i32 i32 i32)))
  (type (;23;) (func (param i32 i64 i64)))
  (type (;24;) (func (param i32 i64 i64) (result i32)))
  (type (;25;) (func (param i32 i64 i64 i32)))
  (type (;26;) (func (param i64)))
  (type (;27;) (func (param i64) (result i64)))
  (type (;28;) (func (param i64 i32 i32) (result i32)))
  (type (;29;) (func (param i64 i64)))
  (type (;30;) (func (param i64 i64) (result i64)))
  (type (;31;) (func (param i64 i64 i32)))
  (type (;32;) (func (param i64 i64 i32) (result i64)))
  (type (;33;) (func (param i64 i64 i32 i32) (result i64)))
  (type (;34;) (func (param i64 i64 i64)))
  (import "ic0" "accept_message" (func (;0;) (type 0)))
  (import "ic0" "call_new" (func (;1;) (type 17)))
  (import "ic0" "call_on_cleanup" (func (;2;) (type 6)))
  (import "ic0" "call_data_append" (func (;3;) (type 6)))
  (import "ic0" "call_with_best_effort_response" (func (;4;) (type 3)))
  (import "ic0" "msg_deadline" (func (;5;) (type 2)))
  (import "ic0" "call_cycles_add" (func (;6;) (type 26)))
  (import "ic0" "call_cycles_add128" (func (;7;) (type 29)))
  (import "ic0" "call_perform" (func (;8;) (type 1)))
  (import "ic0" "msg_arg_data_size" (func (;9;) (type 1)))
  (import "ic0" "msg_arg_data_copy" (func (;10;) (type 8)))
  (import "ic0" "msg_caller_size" (func (;11;) (type 1)))
  (import "ic0" "msg_caller_copy" (func (;12;) (type 8)))
  (import "ic0" "canister_self_size" (func (;13;) (type 1)))
  (import "ic0" "canister_self_copy" (func (;14;) (type 8)))
  (import "ic0" "canister_status" (func (;15;) (type 1)))
  (import "ic0" "msg_reject_msg_size" (func (;16;) (type 1)))
  (import "ic0" "msg_reject_msg_copy" (func (;17;) (type 8)))
  (import "ic0" "msg_reply_data_append" (func (;18;) (type 6)))
  (import "ic0" "msg_reply" (func (;19;) (type 0)))
  (import "ic0" "msg_reject" (func (;20;) (type 6)))
  (import "ic0" "msg_reject_code" (func (;21;) (type 1)))
  (import "ic0" "msg_cycles_available" (func (;22;) (type 2)))
  (import "ic0" "msg_cycles_available128" (func (;23;) (type 3)))
  (import "ic0" "msg_cycles_refunded" (func (;24;) (type 2)))
  (import "ic0" "msg_cycles_refunded128" (func (;25;) (type 3)))
  (import "ic0" "msg_cycles_accept" (func (;26;) (type 27)))
  (import "ic0" "msg_cycles_accept128" (func (;27;) (type 31)))
  (import "ic0" "canister_cycle_balance" (func (;28;) (type 2)))
  (import "ic0" "canister_cycle_balance128" (func (;29;) (type 3)))
  (import "ic0" "stable_size" (func (;30;) (type 1)))
  (import "ic0" "stable64_size" (func (;31;) (type 2)))
  (import "ic0" "stable_grow" (func (;32;) (type 4)))
  (import "ic0" "stable64_grow" (func (;33;) (type 27)))
  (import "ic0" "stable_read" (func (;34;) (type 8)))
  (import "ic0" "stable64_read" (func (;35;) (type 34)))
  (import "ic0" "stable_write" (func (;36;) (type 8)))
  (import "ic0" "stable64_write" (func (;37;) (type 34)))
  (import "ic0" "certified_data_set" (func (;38;) (type 6)))
  (import "ic0" "data_certificate_present" (func (;39;) (type 1)))
  (import "ic0" "data_certificate_size" (func (;40;) (type 1)))
  (import "ic0" "data_certificate_copy" (func (;41;) (type 8)))
  (import "ic0" "time" (func (;42;) (type 2)))
  (import "ic0" "performance_counter" (func (;43;) (type 5)))
  (import "ic0" "msg_method_name_size" (func (;44;) (type 1)))
  (import "ic0" "msg_method_name_copy" (func (;45;) (type 8)))
  (import "ic0" "global_timer_set" (func (;46;) (type 27)))
  (import "ic0" "canister_version" (func (;47;) (type 2)))
  (import "ic0" "debug_print" (func (;48;) (type 6)))
  (import "ic0" "trap" (func (;49;) (type 6)))
  (import "ic0" "mint_cycles" (func (;50;) (type 27)))
  (import "ic0" "is_controller" (func (;51;) (type 7)))
  (import "ic0" "in_replicated_execution" (func (;52;) (type 1)))
  (import "ic0" "cycles_burn128" (func (;53;) (type 31)))
)
"#;
