use crate::flow::{AddErc20TokenFlow, ManagedCanistersAssert};
use crate::metrics::MetricsAssert;
use candid::{Decode, Encode, Nat, Principal};
use ic_cdk::api::management_canister::main::{
    CanisterId, CanisterSettings, CanisterStatusResponse, CanisterStatusType,
};
use ic_ledger_suite_orchestrator::candid::{
    AddErc20Arg, CyclesManagement, Erc20Contract, InitArg, InstalledCanister, InstalledLedgerSuite,
    LedgerInitArg, ManagedCanisterIds, OrchestratorArg, OrchestratorInfo, UpgradeArg,
};
use ic_ledger_suite_orchestrator::state::{
    ArchiveWasm, IndexWasm, LedgerSuiteVersion, LedgerWasm, Wasm, WasmHash,
};
use ic_test_utilities_load_wasm::load_wasm;
pub use icrc_ledger_types::icrc::generic_metadata_value::MetadataValue as LedgerMetadataValue;
pub use icrc_ledger_types::icrc1::account::Account as LedgerAccount;
use pocket_ic::{CallError, PocketIc, PocketIcBuilder, WasmResult};
use std::sync::Arc;

pub mod arbitrary;
pub mod flow;
pub mod metrics;
pub mod universal_canister;

const MAX_TICKS: usize = 10;
const GIT_COMMIT_HASH: &str = "6a8e5fca2c6b4e12966638c444e994e204b42989";
pub const GIT_COMMIT_HASH_UPGRADE: &str = "b7fef0f57ca246b18deda3efd34a24bb605c8199";
pub const CKERC20_TRANSFER_FEE: u64 = 4_000; //0.004 USD for ckUSDC/ckUSDT
pub const DECIMALS: u8 = 6;

pub const NNS_ROOT_PRINCIPAL: Principal = Principal::from_slice(&[0_u8]);
pub const MINTER_PRINCIPAL: Principal =
    Principal::from_slice(&[0_u8, 0, 0, 0, 2, 48, 0, 156, 1, 1]);

pub struct LedgerSuiteOrchestrator {
    pub env: Arc<PocketIc>,
    pub controller: Principal,
    pub ledger_suite_orchestrator_id: CanisterId,
    pub ledger_suite_orchestrator_wasm: Vec<u8>,
    pub embedded_ledger_wasm_hash: WasmHash,
    pub embedded_index_wasm_hash: WasmHash,
    pub embedded_archive_wasm_hash: WasmHash,
}

impl Default for LedgerSuiteOrchestrator {
    fn default() -> Self {
        Self::new(Arc::new(new_state_machine()), default_init_arg()).register_embedded_wasms()
    }
}

impl AsRef<PocketIc> for LedgerSuiteOrchestrator {
    fn as_ref(&self) -> &PocketIc {
        &self.env
    }
}

impl LedgerSuiteOrchestrator {
    pub fn with_cycles_management(cycles_management: CyclesManagement) -> Self {
        let init_arg = InitArg {
            cycles_management: Some(cycles_management),
            ..default_init_arg()
        };
        Self::new(Arc::new(new_state_machine()), init_arg)
    }

    pub fn new(env: Arc<PocketIc>, init_arg: InitArg) -> Self {
        let controller = NNS_ROOT_PRINCIPAL;
        let ledger_suite_orchestrator_id = env.create_canister_with_settings(
            Some(controller),
            Some(CanisterSettings {
                controllers: Some(vec![controller]),
                ..CanisterSettings::default()
            }),
        );
        env.add_cycles(ledger_suite_orchestrator_id, u128::MAX);
        Self {
            env,
            controller,
            ledger_suite_orchestrator_id,
            ledger_suite_orchestrator_wasm: ledger_suite_orchestrator_wasm(),
            embedded_ledger_wasm_hash: ledger_wasm().hash().clone(),
            embedded_index_wasm_hash: index_wasm().hash().clone(),
            embedded_archive_wasm_hash: archive_wasm().hash().clone(),
        }
        .install_ledger_suite_orchestrator(init_arg)
    }

    pub fn new_with_ledger_get_blocks_disabled(env: Arc<PocketIc>, init_arg: InitArg) -> Self {
        let controller = NNS_ROOT_PRINCIPAL;
        let ledger_suite_orchestrator_id = env.create_canister_with_settings(
            Some(controller),
            Some(CanisterSettings {
                controllers: Some(vec![controller]),
                ..CanisterSettings::default()
            }),
        );
        env.add_cycles(ledger_suite_orchestrator_id, u128::MAX);
        Self {
            env,
            controller,
            ledger_suite_orchestrator_id,
            ledger_suite_orchestrator_wasm: ledger_suite_orchestrator_get_blocks_disabled_wasm(),
            embedded_ledger_wasm_hash: ledger_get_blocks_disabled_wasm().hash().clone(),
            embedded_index_wasm_hash: index_wasm().hash().clone(),
            embedded_archive_wasm_hash: archive_wasm().hash().clone(),
        }
        .install_ledger_suite_orchestrator(init_arg)
    }

    fn install_ledger_suite_orchestrator(self, init_arg: InitArg) -> Self {
        self.env.install_canister(
            self.ledger_suite_orchestrator_id,
            self.ledger_suite_orchestrator_wasm.clone(),
            Encode!(&OrchestratorArg::InitArg(init_arg)).unwrap(),
            Some(self.controller),
        );
        self
    }

    pub fn upgrade_ledger_suite_orchestrator_expecting_ok(
        self,
        upgrade_arg: &OrchestratorArg,
    ) -> Self {
        self.upgrade_ledger_suite_orchestrator_with_same_wasm(upgrade_arg)
            .expect("Failed to upgrade ledger suite orchestrator");
        self
    }

    pub fn register_embedded_wasms(self) -> Self {
        self.upgrade_ledger_suite_orchestrator_expecting_ok(&OrchestratorArg::UpgradeArg(
            UpgradeArg {
                git_commit_hash: Some(GIT_COMMIT_HASH.to_string()),
                ledger_compressed_wasm_hash: None,
                index_compressed_wasm_hash: None,
                archive_compressed_wasm_hash: None,
                cycles_management: None,
                manage_ledger_suites: None,
            },
        ))
    }

    pub fn embedded_ledger_suite_version(&self) -> LedgerSuiteVersion {
        LedgerSuiteVersion {
            ledger_compressed_wasm_hash: self.embedded_ledger_wasm_hash.clone(),
            index_compressed_wasm_hash: self.embedded_index_wasm_hash.clone(),
            archive_compressed_wasm_hash: self.embedded_archive_wasm_hash.clone(),
        }
    }

    pub fn upgrade_ledger_suite_orchestrator_with_same_wasm(
        &self,
        upgrade_arg: &OrchestratorArg,
    ) -> Result<(), CallError> {
        self.env.tick(); //tick before upgrade to finish current timers which are reset afterwards
        self.env.upgrade_canister(
            self.ledger_suite_orchestrator_id,
            self.ledger_suite_orchestrator_wasm.clone(),
            Encode!(upgrade_arg).unwrap(),
            Some(self.controller),
        )
    }

    pub fn get_canister_status(&self) -> CanisterStatusResponse {
        self.env
            .canister_status(self.ledger_suite_orchestrator_id, Some(self.controller))
            .unwrap()
    }

    pub fn assert_managed_canisters(self, contract: &Erc20Contract) -> ManagedCanistersAssert {
        let canister_ids = self
            .call_orchestrator_canister_ids(contract)
            .unwrap_or_else(|| panic!("No managed canister IDs found for contract {:?}", contract));

        assert_ne!(
            canister_ids.ledger, canister_ids.index,
            "BUG: ledger and index canister IDs MUST be different"
        );

        ManagedCanistersAssert {
            setup: self,
            canister_ids,
        }
    }

    pub fn add_erc20_token(self, params: AddErc20Arg) -> AddErc20TokenFlow {
        let setup = self.upgrade_ledger_suite_orchestrator_expecting_ok(
            &OrchestratorArg::AddErc20Arg(params.clone()),
        );
        AddErc20TokenFlow { setup, params }
    }

    pub fn manage_installed_canisters(
        self,
        manage_installed_canister: Vec<InstalledLedgerSuite>,
    ) -> Self {
        self.upgrade_ledger_suite_orchestrator_expecting_ok(&OrchestratorArg::UpgradeArg(
            UpgradeArg {
                git_commit_hash: None,
                ledger_compressed_wasm_hash: None,
                index_compressed_wasm_hash: None,
                archive_compressed_wasm_hash: None,
                cycles_management: None,
                manage_ledger_suites: Some(manage_installed_canister),
            },
        ))
    }

    pub fn upgrade_ledger_suite_orchestrator(
        self,
        new_ledger_suite_orchestrator_wasm: Vec<u8>,
        upgrade_arg: UpgradeArg,
    ) -> Self {
        self.env.tick(); //tick before upgrade to finish current timers which are reset afterwards
        let new_embedded_ledger_wasm_hash = upgrade_arg
            .ledger_compressed_wasm_hash
            .clone()
            .map(|s| s.parse().unwrap())
            .unwrap_or(self.embedded_ledger_wasm_hash);
        let new_embedded_index_wasm_hash = upgrade_arg
            .index_compressed_wasm_hash
            .clone()
            .map(|s| s.parse().unwrap())
            .unwrap_or(self.embedded_index_wasm_hash);
        let new_embedded_archive_wasm_hash = upgrade_arg
            .archive_compressed_wasm_hash
            .clone()
            .map(|s| s.parse().unwrap())
            .unwrap_or(self.embedded_archive_wasm_hash);
        self.env
            .upgrade_canister(
                self.ledger_suite_orchestrator_id,
                new_ledger_suite_orchestrator_wasm.clone(),
                Encode!(&OrchestratorArg::UpgradeArg(upgrade_arg)).unwrap(),
                Some(self.controller),
            )
            .expect("Failed to upgrade ERC20");
        Self {
            env: self.env,
            controller: self.controller,
            ledger_suite_orchestrator_id: self.ledger_suite_orchestrator_id,
            ledger_suite_orchestrator_wasm: new_ledger_suite_orchestrator_wasm,
            embedded_ledger_wasm_hash: new_embedded_ledger_wasm_hash,
            embedded_index_wasm_hash: new_embedded_index_wasm_hash,
            embedded_archive_wasm_hash: new_embedded_archive_wasm_hash,
        }
    }

    pub fn call_orchestrator_canister_ids(
        &self,
        contract: &Erc20Contract,
    ) -> Option<ManagedCanisterIds> {
        Decode!(
            &assert_reply(
                self.env
                    .query_call(
                        self.ledger_suite_orchestrator_id,
                        Principal::anonymous(),
                        "canister_ids",
                        Encode!(contract).unwrap()
                    )
                    .expect("failed to execute token transfer")
            ),
            Option<ManagedCanisterIds>
        )
        .unwrap()
    }

    pub fn advance_time_for_periodic_tasks(&self) {
        self.env
            .advance_time(std::time::Duration::from_secs(60 * 60 + 1));
        self.env.tick();
        self.env.tick();
        self.env.tick();
        self.env.tick();
        self.env.tick();
        self.env.tick();
    }

    pub fn advance_time_for_upgrade(&self) {
        self.env.tick();
        self.env.tick();
        self.env.tick();
        self.env.tick();
        self.env.tick();
        self.env.tick();
    }

    pub fn canister_status_of(&self, controlled_canister_id: CanisterId) -> CanisterStatusResponse {
        self.env
            .canister_status(
                controlled_canister_id,
                Some(self.ledger_suite_orchestrator_id),
            )
            .unwrap()
    }

    pub fn cycles_of(&self, canister_id: CanisterId) -> u128 {
        use num_traits::cast::ToPrimitive;
        self.canister_status_of(canister_id)
            .cycles
            .0
            .to_u128()
            .unwrap()
    }

    pub fn get_orchestrator_info(&self) -> OrchestratorInfo {
        Decode!(
            &assert_reply(
                self.env
                    .query_call(
                        self.ledger_suite_orchestrator_id,
                        Principal::anonymous(),
                        "get_orchestrator_info",
                        Encode!().unwrap()
                    )
                    .unwrap()
            ),
            OrchestratorInfo
        )
        .unwrap()
    }

    pub fn check_metrics(self) -> MetricsAssert<Self> {
        let canister_id = self.ledger_suite_orchestrator_id;
        MetricsAssert::from_querying_metrics(self, canister_id)
    }
}

pub fn default_init_arg() -> InitArg {
    InitArg {
        more_controller_ids: vec![NNS_ROOT_PRINCIPAL],
        minter_id: Some(MINTER_PRINCIPAL),
        cycles_management: None,
    }
}

pub fn new_state_machine() -> PocketIc {
    PocketIcBuilder::new().with_fiduciary_subnet().build()
}

pub fn ledger_suite_orchestrator_wasm() -> Vec<u8> {
    load_wasm(
        std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        "ledger_suite_orchestrator",
        &[],
    )
}

pub fn ledger_suite_orchestrator_get_blocks_disabled_wasm() -> Vec<u8> {
    load_wasm(
        std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        "ledger_suite_orchestrator_get_blocks_disabled",
        &[],
    )
}

pub fn ledger_wasm() -> LedgerWasm {
    LedgerWasm::from(load_wasm(
        std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        "ledger_canister",
        &[],
    ))
}

fn ledger_get_blocks_disabled_wasm() -> LedgerWasm {
    LedgerWasm::from(load_wasm(
        std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        "ledger_canister_get_blocks_disabled",
        &[],
    ))
}

pub fn index_wasm() -> IndexWasm {
    IndexWasm::from(load_wasm(
        std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        "index_canister",
        &[],
    ))
}

fn archive_wasm() -> ArchiveWasm {
    ArchiveWasm::from(load_wasm(
        std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        "ledger_archive_node_canister",
        &[],
    ))
}

fn is_gzipped_blob(blob: &[u8]) -> bool {
    (blob.len() > 4)
        // Has magic bytes.
        && (blob[0..2] == [0x1F, 0x8B])
}

fn modify_wasm<T>(wasm: Wasm<T>) -> Wasm<T> {
    let wasm_bytes = wasm.to_bytes();
    // wasm_bytes are gzipped and the subslice [4..8]
    // is the little endian representation of a timestamp
    // so we just flip a bit in the timestamp
    assert!(is_gzipped_blob(&wasm_bytes));
    let mut new_wasm_bytes = wasm_bytes.clone();
    *new_wasm_bytes.get_mut(7).expect("cannot be empty") ^= 1;
    assert_ne!(wasm_bytes, new_wasm_bytes);
    Wasm::from(new_wasm_bytes)
}

pub fn tweak_ledger_suite_wasms() -> (LedgerWasm, IndexWasm, ArchiveWasm) {
    (
        LedgerWasm::from(modify_wasm(ledger_wasm())),
        IndexWasm::from(modify_wasm(index_wasm())),
        ArchiveWasm::from(modify_wasm(archive_wasm())),
    )
}

pub fn supported_erc20_tokens() -> Vec<AddErc20Arg> {
    vec![usdc(), usdt()]
}

pub fn usdc() -> AddErc20Arg {
    AddErc20Arg {
        contract: usdc_erc20_contract(),
        ledger_init_arg: ledger_init_arg("Chain-Key USD Coin", "ckUSDC"),
    }
}

pub fn usdc_erc20_contract() -> Erc20Contract {
    Erc20Contract {
        chain_id: Nat::from(1_u8),
        address: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
    }
}

pub fn usdt() -> AddErc20Arg {
    AddErc20Arg {
        contract: usdt_erc20_contract(),
        ledger_init_arg: ledger_init_arg("Chain-Key Tether USD", "ckUSDT"),
    }
}

pub fn usdt_erc20_contract() -> Erc20Contract {
    Erc20Contract {
        chain_id: Nat::from(1_u8),
        address: "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string(),
    }
}

pub fn cketh_installed_canisters() -> InstalledLedgerSuite {
    InstalledLedgerSuite {
        token_symbol: "ckETH".to_string(),
        ledger: InstalledCanister {
            canister_id: "ss2fx-dyaaa-aaaar-qacoq-cai".parse().unwrap(),
            installed_wasm_hash: "8457289d3b3179aa83977ea21bfa2fc85e402e1f64101ecb56a4b963ed33a1e6"
                .to_string(),
        },
        index: InstalledCanister {
            canister_id: "s3zol-vqaaa-aaaar-qacpa-cai".parse().unwrap(),
            installed_wasm_hash: "eb3096906bf9a43996d2ca9ca9bfec333a402612f132876c8ed1b01b9844112a"
                .to_string(),
        },
        archives: Some(vec!["xob7s-iqaaa-aaaar-qacra-cai".parse().unwrap()]),
    }
}

fn ledger_init_arg<U: Into<String>, V: Into<String>>(
    token_name: U,
    token_symbol: V,
) -> LedgerInitArg {
    LedgerInitArg {
        transfer_fee: CKERC20_TRANSFER_FEE.into(),
        decimals: DECIMALS,
        token_name: token_name.into(),
        token_symbol: token_symbol.into(),
        token_logo: "".to_string(),
    }
}

pub fn assert_reply(result: WasmResult) -> Vec<u8> {
    match result {
        WasmResult::Reply(bytes) => bytes,
        WasmResult::Reject(reject) => {
            panic!("Expected a successful reply, got a reject: {}", reject)
        }
    }
}

pub fn out_of_band_upgrade<T: AsRef<PocketIc>>(
    env: T,
    controller: Principal,
    target: CanisterId,
    wasm: Vec<u8>,
) -> Result<(), CallError> {
    env.as_ref()
        .upgrade_canister(target, wasm, Encode!(&()).unwrap(), Some(controller))
}

pub fn stop_canister<T: AsRef<PocketIc>, P: Into<Principal>>(
    env: T,
    controller: P,
    target: CanisterId,
) {
    let controller = controller.into();
    env.as_ref()
        .stop_canister(target, Some(controller))
        .expect("stopping canister failed");
    let status = env
        .as_ref()
        .canister_status(target, Some(controller))
        .unwrap()
        .status;
    assert_eq!(status, CanisterStatusType::Stopped);
}
