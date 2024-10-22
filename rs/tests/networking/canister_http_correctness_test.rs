/* tag::catalog[]
Title:: Test correctness of feature according to spec.

Goal:: Ensure simple HTTP requests can be made from canisters.

Runbook::
0. Instantiate a universal VM with a webserver
1. Instantiate an IC with one application subnet with the HTTP feature enabled.
2. Install NNS canisters
3. Install the proxy canister
4. Make an update call to the proxy canister.

Success::
1. Received http response with status 200.

end::catalog[] */

use anyhow::bail;
use anyhow::Result;
use canister_http::*;
use canister_test::{Canister, Runtime};
use dfn_candid::candid_one;
use ic_base_types::{CanisterId, NumBytes};
use ic_cdk::api::call::RejectionCode;
use ic_management_canister_types::{
    BoundedHttpHeaders, CanisterHttpRequestArgs, HttpHeader, HttpMethod, TransformContext,
    TransformFunc,
};
use ic_system_test_driver::driver::group::SystemTestGroup;
use ic_system_test_driver::driver::group::SystemTestSubGroup;
use ic_system_test_driver::driver::{test_env::TestEnv, test_env_api::RETRY_BACKOFF};
use ic_system_test_driver::systest;
use ic_system_test_driver::util::block_on;
use ic_test_utilities::cycles_account_manager::CyclesAccountManagerBuilder;
use ic_test_utilities_types::messages::RequestBuilder;
use ic_types::canister_http::{CanisterHttpRequestContext, MAX_CANISTER_HTTP_REQUEST_BYTES};
use ic_types::time::UNIX_EPOCH;
use proxy_canister::{RemoteHttpRequest, RemoteHttpResponse};
use slog::Logger;
use std::convert::TryFrom;
use std::time::Duration;

struct Handlers<'a> {
    logger: Logger,
    subnet_size: usize,
    runtime: Runtime,
    env: &'a TestEnv,
}

impl<'a> Handlers<'a> {
    fn new(env: &'a TestEnv) -> Handlers<'a> {
        let logger = env.logger();
        let subnet_size = get_node_snapshots(&env).count();

        let runtime = {
            let mut nodes = get_node_snapshots(&env);
            let node = nodes.next().expect("there is no application node");
            get_runtime_from_node(&node)
        };

        Handlers {
            logger,
            runtime,
            subnet_size,
            env,
        }
    }

    fn proxy_canister(&self) -> Canister<'_> {
        let principal_id = get_proxy_canister_id(self.env);
        let canister_id = CanisterId::unchecked_from_principal(principal_id);
        Canister::new(&self.runtime, canister_id)
    }
}

fn main() -> Result<()> {
    SystemTestGroup::new()
        .with_setup(canister_http::setup)
        .add_parallel(
            SystemTestSubGroup::new()
                .add_test(systest!(test_enforce_https))
                .add_test(systest!(test_transform_function_is_executed))
                .add_test(systest!(test_composite_transform_function_is_executed))
                .add_test(systest!(test_no_cycles_attached))
                .add_test(systest!(test_2mb_response_cycle_for_success_path))
                .add_test(systest!(test_2mb_response_cycle_for_rejection_path))
                .add_test(systest!(test_4096_max_response_cycle_case_1))
                .add_test(systest!(test_4096_max_response_cycle_case_2))
                .add_test(systest!(test_max_response_limit_too_large))
                .add_test(systest!(
                    test_transform_that_bloats_response_above_2mb_limit
                ))
                .add_test(systest!(test_non_existing_transform_function))
                .add_test(systest!(test_post_request))
                .add_test(systest!(test_http_endpoint_response_is_too_large))
                .add_test(systest!(
                    test_http_endpoint_with_delayed_response_is_rejected
                ))
                .add_test(systest!(test_that_redirects_are_not_followed))
                .add_test(systest!(test_http_calls_to_ic_fails)),
        )
        .execute_from_args()?;

    Ok(())
}

pub fn test_enforce_https(env: TestEnv) {
    let handlers = Handlers::new(&env);
    let webserver_ipv6 = get_universal_vm_address(&env);

    block_on(async {
        assert!(
            test_canister_http_property(
                &handlers.logger,
                &handlers.proxy_canister(),
                RemoteHttpRequest {
                    request: CanisterHttpRequestArgs {
                        url: format!("http://[{webserver_ipv6}]:20443"),
                        headers: BoundedHttpHeaders::new(vec![]),
                        method: HttpMethod::GET,
                        body: Some("".as_bytes().to_vec()),
                        transform: Some(TransformContext {
                            function: TransformFunc(candid::Func {
                                principal: get_proxy_canister_id(&env).into(),
                                method: "transform".to_string(),
                            }),
                            context: vec![0, 1, 2],
                        }),
                        max_response_bytes: None,
                    },
                    cycles: 500_000_000_000,
                },
                |response| matches!(response, Err((RejectionCode::SysFatal, _))),
            )
            .await
        )
    });
}

pub fn test_transform_function_is_executed(env: TestEnv) {
    let handlers = Handlers::new(&env);
    let webserver_ipv6 = get_universal_vm_address(&env);

    block_on(async {
        assert!(
            test_canister_http_property(
                &handlers.logger,
                &handlers.proxy_canister(),
                RemoteHttpRequest {
                    request: CanisterHttpRequestArgs {
                        url: format!("https://[{webserver_ipv6}]:20443"),
                        headers: BoundedHttpHeaders::new(vec![]),
                        method: HttpMethod::GET,
                        body: Some("".as_bytes().to_vec()),
                        transform: Some(TransformContext {
                            function: TransformFunc(candid::Func {
                                principal: get_proxy_canister_id(&env).into(),
                                method: "test_transform".to_string(),
                            }),
                            context: vec![0, 1, 2],
                        }),
                        max_response_bytes: None,
                    },
                    cycles: 500_000_000_000,
                },
                |response| {
                    let r = response.clone().expect("Http call should succeed");
                    r.headers.len() == 2
                        && r.headers[0].0 == "hello"
                        && r.headers[0].1 == "bonjour"
                        && r.headers[1].0 == "caller"
                        && r.headers[1].1 == "aaaaa-aa"
                },
            )
            .await
        )
    });
}

pub fn test_composite_transform_function_is_executed(env: TestEnv) {
    let handlers = Handlers::new(&env);
    let webserver_ipv6 = get_universal_vm_address(&env);

    block_on(async {
        assert!(
            test_canister_http_property(
                &handlers.logger,
                &handlers.proxy_canister(),
                RemoteHttpRequest {
                    request: CanisterHttpRequestArgs {
                        url: format!("https://[{webserver_ipv6}]:20443"),
                        headers: BoundedHttpHeaders::new(vec![]),
                        method: HttpMethod::GET,
                        body: Some("".as_bytes().to_vec()),
                        transform: Some(TransformContext {
                            function: TransformFunc(candid::Func {
                                principal: get_proxy_canister_id(&env).into(),
                                method: "test_composite_transform".to_string(),
                            }),
                            context: vec![0, 1, 2],
                        }),
                        max_response_bytes: None,
                    },
                    cycles: 500_000_000_000,
                },
                |response| {
                    let r = response.clone().expect("Http call should succeed");
                    r.headers.len() == 2
                        && r.headers[0].0 == "hello"
                        && r.headers[0].1 == "bonjour"
                        && r.headers[1].0 == "caller"
                        && r.headers[1].1 == "aaaaa-aa"
                },
            )
            .await,
        )
    });
}

pub fn test_no_cycles_attached(env: TestEnv) {
    let handlers = Handlers::new(&env);
    let webserver_ipv6 = get_universal_vm_address(&env);

    block_on(async {
        assert!(
            test_canister_http_property(
                &handlers.logger,
                &handlers.proxy_canister(),
                RemoteHttpRequest {
                    request: CanisterHttpRequestArgs {
                        url: format!("http://[{webserver_ipv6}]:20443"),
                        headers: BoundedHttpHeaders::new(vec![]),
                        method: HttpMethod::GET,
                        body: Some("".as_bytes().to_vec()),
                        transform: Some(TransformContext {
                            function: TransformFunc(candid::Func {
                                principal: get_proxy_canister_id(&env).into(),
                                method: "transform".to_string(),
                            }),
                            context: vec![0, 1, 2],
                        }),
                        max_response_bytes: None,
                    },
                    cycles: 0,
                },
                |response| matches!(response, Err((RejectionCode::CanisterReject, _))),
            )
            .await,
        )
    });
}

pub fn test_2mb_response_cycle_for_success_path(env: TestEnv) {
    let handlers = Handlers::new(&env);
    let webserver_ipv6 = get_universal_vm_address(&env);

    block_on(async {
        // Test: Pricing without max_response specified
        // Formula: 400M + (2*response_size_limit + 2*request_size) * 50000
        let request = CanisterHttpRequestArgs {
            url: format!("https://[{webserver_ipv6}]:20443"),
            headers: BoundedHttpHeaders::new(vec![]),
            method: HttpMethod::GET,
            body: Some("".as_bytes().to_vec()),
            transform: Some(TransformContext {
                function: TransformFunc(candid::Func {
                    principal: get_proxy_canister_id(&env).into(),
                    method: "transform".to_string(),
                }),
                context: vec![0, 1, 2],
            }),
            max_response_bytes: None,
        };

        assert!(
            test_canister_http_property(
                &handlers.logger,
                &handlers.proxy_canister(),
                RemoteHttpRequest {
                    request: request.clone(),
                    cycles: expected_cycle_cost(
                        handlers.proxy_canister().canister_id(),
                        request,
                        handlers.subnet_size,
                    ),
                },
                |response| matches!(response, Ok(r) if r.status==200),
            )
            .await,
        )
    });
}

pub fn test_2mb_response_cycle_for_rejection_path(env: TestEnv) {
    let handlers = Handlers::new(&env);
    let webserver_ipv6 = get_universal_vm_address(&env);

    block_on(async {
        // Test: Pricing without max_response specified
        // Formula: 400M + (2*response_size_limit + 2*request_size) * 50000
        let request = CanisterHttpRequestArgs {
            url: format!("https://[{webserver_ipv6}]:20443"),
            headers: BoundedHttpHeaders::new(vec![]),
            method: HttpMethod::GET,
            body: Some("".as_bytes().to_vec()),
            transform: Some(TransformContext {
                function: TransformFunc(candid::Func {
                    principal: get_proxy_canister_id(&env).into(),
                    method: "transform".to_string(),
                }),
                context: vec![0, 1, 2],
            }),
            max_response_bytes: None,
        };

        assert!(
            test_canister_http_property(
                &handlers.logger,
                &handlers.proxy_canister(),
                RemoteHttpRequest {
                    request: request.clone(),
                    cycles: expected_cycle_cost(
                        handlers.proxy_canister().canister_id(),
                        request,
                        handlers.subnet_size,
                    ) - 1,
                },
                |response| matches!(response, Err((RejectionCode::CanisterReject, _))),
            )
            .await,
        )
    });
}

pub fn test_4096_max_response_cycle_case_1(env: TestEnv) {
    let handlers = Handlers::new(&env);
    let webserver_ipv6 = get_universal_vm_address(&env);

    // Test: Priceing with max response size specified
    // Formula: 400M + (2*response_size_limit + 2*request_size) * 50000
    let request = CanisterHttpRequestArgs {
        url: format!("https://[{webserver_ipv6}]:20443"),
        headers: BoundedHttpHeaders::new(vec![]),
        method: HttpMethod::GET,
        body: Some("".as_bytes().to_vec()),
        transform: Some(TransformContext {
            function: TransformFunc(candid::Func {
                principal: get_proxy_canister_id(&env).into(),
                method: "transform".to_string(),
            }),
            context: vec![0, 1, 2],
        }),
        max_response_bytes: Some(16384),
    };

    block_on(async {
        // Test: Pricing without max_response specified
        // Formula: 400M + (2*response_size_limit + 2*request_size) * 50000

        assert!(
            test_canister_http_property(
                &handlers.logger,
                &handlers.proxy_canister(),
                RemoteHttpRequest {
                    request: request.clone(),
                    cycles: expected_cycle_cost(
                        handlers.proxy_canister().canister_id(),
                        request.clone(),
                        handlers.subnet_size,
                    ),
                },
                |response| matches!(response, Ok(r) if r.status==200),
            )
            .await,
        )
    });
}

pub fn test_4096_max_response_cycle_case_2(env: TestEnv) {
    let handlers = Handlers::new(&env);
    let webserver_ipv6 = get_universal_vm_address(&env);

    // Test: Priceing with max response size specified
    // Formula: 400M + (2*response_size_limit + 2*request_size) * 50000
    let request = CanisterHttpRequestArgs {
        url: format!("https://[{webserver_ipv6}]:20443"),
        headers: BoundedHttpHeaders::new(vec![]),
        method: HttpMethod::GET,
        body: Some("".as_bytes().to_vec()),
        transform: Some(TransformContext {
            function: TransformFunc(candid::Func {
                principal: get_proxy_canister_id(&env).into(),
                method: "transform".to_string(),
            }),
            context: vec![0, 1, 2],
        }),
        max_response_bytes: Some(16384),
    };

    block_on(async {
        // Test: Pricing without max_response specified
        // Formula: 400M + (2*response_size_limit + 2*request_size) * 50000

        assert!(
            test_canister_http_property(
                &handlers.logger,
                &handlers.proxy_canister(),
                RemoteHttpRequest {
                    request: request.clone(),
                    cycles: expected_cycle_cost(
                        handlers.proxy_canister().canister_id(),
                        request.clone(),
                        handlers.subnet_size,
                    ) - 1,
                },
                |response| matches!(response, Err((RejectionCode::CanisterReject, _))),
            )
            .await,
        )
    });
}

pub fn test_max_response_limit_too_large(env: TestEnv) {
    let handlers = Handlers::new(&env);
    let webserver_ipv6 = get_universal_vm_address(&env);

    block_on(async {
        assert!(
            test_canister_http_property(
                &handlers.logger,
                &handlers.proxy_canister(),
                RemoteHttpRequest {
                    request: CanisterHttpRequestArgs {
                        url: format!("https://[{webserver_ipv6}]:20443"),
                        headers: BoundedHttpHeaders::new(vec![]),
                        method: HttpMethod::GET,
                        body: Some("".as_bytes().to_vec()),
                        transform: Some(TransformContext {
                            function: TransformFunc(candid::Func {
                                principal: get_proxy_canister_id(&env).into(),
                                method: "transform".to_string(),
                            }),
                            context: vec![0, 1, 2],
                        }),
                        max_response_bytes: Some(4 * 1024 * 1024),
                    },
                    cycles: 0,
                },
                |response| matches!(response, Err((RejectionCode::CanisterReject, _))),
            )
            .await,
        )
    });
}

pub fn test_transform_that_bloats_response_above_2mb_limit(env: TestEnv) {
    let handlers = Handlers::new(&env);
    let webserver_ipv6 = get_universal_vm_address(&env);

    block_on(async {
        assert!(
            test_canister_http_property(
                &handlers.logger,
                &handlers.proxy_canister(),
                RemoteHttpRequest {
                    request: CanisterHttpRequestArgs {
                        url: format!("https://[{webserver_ipv6}]:20443"),
                        headers: BoundedHttpHeaders::new(vec![]),
                        method: HttpMethod::GET,
                        body: Some("".as_bytes().to_vec()),
                        transform: Some(TransformContext {
                            function: TransformFunc(candid::Func {
                                principal: get_proxy_canister_id(&env).into(),
                                method: "bloat_transform".to_string(),
                            }),
                            context: vec![0, 1, 2],
                        }),
                        max_response_bytes: None,
                    },
                    cycles: 500_000_000_000,
                },
                |response| matches!(response, Err((RejectionCode::SysFatal, _))),
            )
            .await,
        )
    });
}

pub fn test_non_existing_transform_function(env: TestEnv) {
    let handlers = Handlers::new(&env);
    let webserver_ipv6 = get_universal_vm_address(&env);

    block_on(async {
        assert!(
            test_canister_http_property(
                &handlers.logger,
                &handlers.proxy_canister(),
                RemoteHttpRequest {
                    request: CanisterHttpRequestArgs {
                        url: format!("https://[{webserver_ipv6}]:20443"),
                        headers: BoundedHttpHeaders::new(vec![]),
                        method: HttpMethod::GET,
                        body: Some("".as_bytes().to_vec()),
                        transform: Some(TransformContext {
                            function: TransformFunc(candid::Func {
                                principal: get_proxy_canister_id(&env).into(),
                                method: "idontexist".to_string(),
                            }),
                            context: vec![0, 1, 2],
                        }),
                        max_response_bytes: None,
                    },
                    cycles: 500_000_000_000,
                },
                |response| matches!(response, Err((RejectionCode::CanisterError, _))),
            )
            .await,
        )
    });
}

pub fn test_post_request(env: TestEnv) {
    let handlers = Handlers::new(&env);
    let webserver_ipv6 = get_universal_vm_address(&env);

    block_on(async {
        assert!(
            test_canister_http_property(
                &handlers.logger,
                &handlers.proxy_canister(),
                RemoteHttpRequest {
                    request: CanisterHttpRequestArgs {
                        url: format!("https://[{webserver_ipv6}]:20443/post"),
                        headers: BoundedHttpHeaders::new(vec![HttpHeader {
                            name: "Content-Type".to_string(),
                            value: "application/x-www-form-urlencoded".to_string(),
                        }]),
                        method: HttpMethod::POST,
                        body: Some("satoshi=me".as_bytes().to_vec()),
                        transform: Some(TransformContext {
                            function: TransformFunc(candid::Func {
                                principal: get_proxy_canister_id(&env).into(),
                                method: "transform".to_string(),
                            }),
                            context: vec![0, 1, 2],
                        }),
                        max_response_bytes: None,
                    },
                    cycles: 500_000_000_000,
                },
                |response| matches!(response, Ok(r) if r.body.contains("satoshi")),
            )
            .await,
        )
    });
}

pub fn test_http_endpoint_response_is_too_large(env: TestEnv) {
    let handlers = Handlers::new(&env);
    let webserver_ipv6 = get_universal_vm_address(&env);

    block_on(async {
        assert!(
            test_canister_http_property(
                &handlers.logger,
                &handlers.proxy_canister(),
                RemoteHttpRequest {
                    request: CanisterHttpRequestArgs {
                        url: format!("https://[{webserver_ipv6}]:20443/bytes/100000"),
                        headers: BoundedHttpHeaders::new(vec![]),
                        method: HttpMethod::GET,
                        body: Some("".as_bytes().to_vec()),
                        transform: Some(TransformContext {
                            function: TransformFunc(candid::Func {
                                principal: get_proxy_canister_id(&env).into(),
                                method: "transform".to_string(),
                            }),
                            context: vec![0, 1, 2],
                        }),
                        max_response_bytes: Some(8 * 1024),
                    },
                    cycles: 500_000_000_000,
                },
                |response| matches!(response, Err((RejectionCode::SysFatal, _))),
            )
            .await,
        )
    });
}

pub fn test_http_endpoint_with_delayed_response_is_rejected(env: TestEnv) {
    let handlers = Handlers::new(&env);
    let webserver_ipv6 = get_universal_vm_address(&env);

    block_on(async {
        assert!(
            test_canister_http_property(
                &handlers.logger,
                &handlers.proxy_canister(),
                RemoteHttpRequest {
                    request: CanisterHttpRequestArgs {
                        url: format!("https://[{webserver_ipv6}]:20443/delay/40"),
                        headers: BoundedHttpHeaders::new(vec![]),
                        method: HttpMethod::GET,
                        body: Some("".as_bytes().to_vec()),
                        transform: Some(TransformContext {
                            function: TransformFunc(candid::Func {
                                principal: get_proxy_canister_id(&env).into(),
                                method: "transform".to_string(),
                            }),
                            context: vec![0, 1, 2],
                        }),
                        max_response_bytes: None,
                    },
                    cycles: 500_000_000_000,
                },
                |response| matches!(response, Err((RejectionCode::SysFatal, _))),
            )
            .await,
        )
    });
}

/// The adapter should not follow HTTP redirects.
pub fn test_that_redirects_are_not_followed(env: TestEnv) {
    let handlers = Handlers::new(&env);
    let webserver_ipv6 = get_universal_vm_address(&env);

    block_on(async {
        assert!(
            test_canister_http_property(
                &handlers.logger,
                &handlers.proxy_canister(),
                RemoteHttpRequest {
                    request: CanisterHttpRequestArgs {
                        url: format!("https://[{webserver_ipv6}]:20443/redirect/10"),
                        headers: BoundedHttpHeaders::new(vec![]),
                        method: HttpMethod::GET,
                        body: Some("".as_bytes().to_vec()),
                        transform: Some(TransformContext {
                            function: TransformFunc(candid::Func {
                                principal: get_proxy_canister_id(&env).into(),
                                method: "transform".to_string(),
                            }),
                            context: vec![0, 1, 2],
                        }),
                        max_response_bytes: None,
                    },
                    cycles: 500_000_000_000,
                },
                |response| matches!(response, Ok(r) if r.status == 303),
            )
            .await,
        )
    });
}

/// The adapter should not reject HTTP calls that are made to other IC replicas' HTTPS endpoints.
pub fn test_http_calls_to_ic_fails(env: TestEnv) {
    let handlers = Handlers::new(&env);
    let webserver_ipv6 = get_universal_vm_address(&env);

    block_on(async {
        assert!(
            test_canister_http_property(
                &handlers.logger,
                &handlers.proxy_canister(),
                RemoteHttpRequest {
                    request: CanisterHttpRequestArgs {
                        url: format!("https://[{}]:9090", webserver_ipv6),
                        headers: BoundedHttpHeaders::new(vec![]),
                        method: HttpMethod::GET,
                        body: Some("".as_bytes().to_vec()),
                        transform: Some(TransformContext {
                            function: TransformFunc(candid::Func {
                                principal: get_proxy_canister_id(&env).into(),
                                method: "transform".to_string(),
                            }),
                            context: vec![0, 1, 2],
                        }),
                        max_response_bytes: None,
                    },
                    cycles: 500_000_000_000,
                },
                |response| {
                    let err_response = response.clone().unwrap_err();
                    matches!(err_response.0, RejectionCode::SysTransient)
                        && err_response.1.contains("client error (Connect)")
                },
            )
            .await,
        )
    });
}

async fn test_canister_http_property<F>(
    logger: &Logger,
    proxy_canister: &Canister<'_>,
    request: RemoteHttpRequest,
    response_check: F,
) -> bool
where
    F: Fn(&Result<RemoteHttpResponse, (RejectionCode, String)>) -> bool,
{
    let test_result = ic_system_test_driver::retry_with_msg_async!(
        format!(
            "checking send_request of proxy canister {}",
            proxy_canister.canister_id()
        ),
        logger,
        Duration::from_secs(60),
        RETRY_BACKOFF,
        || async {
            let res = proxy_canister
                .update_(
                    "send_request",
                    candid_one::<
                        Result<RemoteHttpResponse, (RejectionCode, String)>,
                        RemoteHttpRequest,
                    >,
                    request.clone(),
                )
                .await
                .expect("Update call to proxy canister failed");
            if !response_check(&res) {
                bail!("Http request didn't pass check: {:?}", &res);
            }
            Ok(())
        }
    )
    .await;

    test_result.is_ok()
}

/// Pricing function of canister http requests.
fn expected_cycle_cost(
    proxy_canister: CanisterId,
    request: CanisterHttpRequestArgs,
    subnet_size: usize,
) -> u64 {
    let cm = CyclesAccountManagerBuilder::new().build();
    let response_size = request
        .max_response_bytes
        .unwrap_or(MAX_CANISTER_HTTP_REQUEST_BYTES);

    let dummy_context = CanisterHttpRequestContext::try_from((
        UNIX_EPOCH,
        &RequestBuilder::default()
            .receiver(CanisterId::from(1))
            .sender(proxy_canister)
            .build(),
        request,
    ))
    .unwrap();
    let req_size = dummy_context.variable_parts_size();
    let cycle_fee = cm.http_request_fee(req_size, Some(NumBytes::from(response_size)), subnet_size);
    cycle_fee.get().try_into().unwrap()
}
