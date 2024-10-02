use crate::NervousSystemError;
use async_trait::async_trait;
use dfn_core::{api::PrincipalId, call, CanisterId};
use dfn_protobuf::protobuf;
use ic_crypto_sha2::Sha256;
use ic_ledger_core::block::BlockIndex;
use icp_ledger::{
    tokens_from_proto, AccountBalanceArgs, AccountIdentifier, Memo, SendArgs,
    Subaccount as IcpSubaccount, Tokens, TotalSupplyArgs,
};
use icrc_ledger_types::icrc1::account::{Account, Subaccount};
use mockall::automock;

#[cfg(feature = "tla")]
use crate::tla::{
    self, account_to_tla, opt_subaccount_to_tla, Destination, ToTla, TLA_INSTRUMENTATION_STATE,
};
#[cfg(feature = "tla")]
use std::collections::BTreeMap;

use crate::{tla_log_request, tla_log_response};

pub struct IcpLedgerCanister {
    id: CanisterId,
}

impl IcpLedgerCanister {
    pub fn new(id: CanisterId) -> Self {
        IcpLedgerCanister { id }
    }
}

/// A trait defining common patterns for accessing the ICRC1 Ledger canister.
#[automock]
#[async_trait]
pub trait ICRC1Ledger: Send + Sync {
    /// Transfers funds from one of this canister's subaccount to
    /// the provided account.
    ///
    /// Returns the block height at which the transfer was recorded.
    async fn transfer_funds(
        &self,
        amount_e8s: u64,
        fee_e8s: u64,
        from_subaccount: Option<Subaccount>,
        to: Account,
        memo: u64,
    ) -> Result<BlockIndex, NervousSystemError>;

    /// Gets the total supply of tokens from the sum of all accounts except for the
    /// minting canister's.
    async fn total_supply(&self) -> Result<Tokens, NervousSystemError>;

    /// Gets the account balance in Tokens of the given AccountIdentifier in the Ledger.
    async fn account_balance(&self, account: Account) -> Result<Tokens, NervousSystemError>;

    /// Returns the CanisterId of the Ledger being accessed.
    fn canister_id(&self) -> CanisterId;
}

/// A trait defining common patterns for accessing the Ledger canister.
#[automock]
#[async_trait]
pub trait IcpLedger: Send + Sync {
    /// Transfers funds from one of this canister's subaccount to
    /// the provided account.
    ///
    /// Returns the block height at which the transfer was recorded.
    async fn transfer_funds(
        &self,
        amount_e8s: u64,
        fee_e8s: u64,
        from_subaccount: Option<IcpSubaccount>,
        to: AccountIdentifier,
        memo: u64,
    ) -> Result<u64, NervousSystemError>;

    /// Gets the total supply of tokens from the sum of all accounts except for the
    /// minting canister's.
    async fn total_supply(&self) -> Result<Tokens, NervousSystemError>;

    /// Gets the account balance in Tokens of the given AccountIdentifier in the Ledger.
    async fn account_balance(
        &self,
        account: AccountIdentifier,
    ) -> Result<Tokens, NervousSystemError>;

    /// Returns the CanisterId of the Ledger being accessed.
    fn canister_id(&self) -> CanisterId;
}

fn icrc1_account_to_icp_accountidentifier(account: Account) -> AccountIdentifier {
    AccountIdentifier::new(account.owner.into(), account.subaccount.map(IcpSubaccount))
}

#[async_trait]
impl ICRC1Ledger for IcpLedgerCanister {
    async fn transfer_funds(
        &self,
        amount_e8s: u64,
        fee_e8s: u64,
        from_subaccount: Option<Subaccount>,
        to: Account,
        memo: u64,
    ) -> Result<BlockIndex, NervousSystemError> {
        <IcpLedgerCanister as IcpLedger>::transfer_funds(
            self,
            amount_e8s,
            fee_e8s,
            from_subaccount.map(IcpSubaccount),
            icrc1_account_to_icp_accountidentifier(to),
            memo,
        )
        .await
    }

    async fn total_supply(&self) -> Result<Tokens, NervousSystemError> {
        <IcpLedgerCanister as IcpLedger>::total_supply(self).await
    }

    async fn account_balance(&self, account: Account) -> Result<Tokens, NervousSystemError> {
        <IcpLedgerCanister as IcpLedger>::account_balance(
            self,
            icrc1_account_to_icp_accountidentifier(account),
        )
        .await
    }

    fn canister_id(&self) -> CanisterId {
        self.id
    }
}

#[async_trait]
impl IcpLedger for IcpLedgerCanister {
    async fn transfer_funds(
        &self,
        amount_e8s: u64,
        fee_e8s: u64,
        from_subaccount: Option<IcpSubaccount>,
        to: AccountIdentifier,
        memo: u64,
    ) -> Result<u64, NervousSystemError> {
        tla_log_request!(
            "WaitForTransfer",
            Destination::new("ledger"),
            "Transfer",
            tla::TlaValue::Record(BTreeMap::from([
                ("amount".to_string(), amount_e8s.to_tla_value()),
                ("fee".to_string(), fee_e8s.to_tla_value()),
                ("from".to_string(), opt_subaccount_to_tla(&from_subaccount)),
                ("to".to_string(), account_to_tla(to)),
            ]))
        );

        // Send 'amount_e8s' to the target account.
        //
        // We expect the 'fee_e8s' AND 'amount_e8s' to be
        // deducted from the from_subaccount. When calling
        // this method, make sure that the staked amount
        // can cover BOTH of these amounts, otherwise there
        // will be an error.
        let result: Result<u64, (Option<i32>, String)> = call(
            self.id,
            "send_pb",
            protobuf,
            SendArgs {
                memo: Memo(memo),
                amount: Tokens::from_e8s(amount_e8s),
                fee: Tokens::from_e8s(fee_e8s),
                from_subaccount,
                to,
                created_at_time: None,
            },
        )
        .await;
        tla_log_response!(
            Destination::new("ledger"),
            if result.is_err() {
                tla::TlaValue::Variant {
                    tag: "Fail".to_string(),
                    value: Box::new(tla::TlaValue::Constant("UNIT".to_string())),
                }
            } else {
                tla::TlaValue::Variant {
                    tag: "TransferOk".to_string(),
                    value: Box::new(tla::TlaValue::Constant("UNIT".to_string())),
                }
            }
        );

        result.map_err(|(code, msg)| {
            NervousSystemError::new_with_message(format!(
                "Error calling method 'send' of the ledger canister. Code: {:?}. Message: {}",
                code, msg
            ))
        })
    }

    async fn total_supply(&self) -> Result<Tokens, NervousSystemError> {
        let result: Result<Tokens, (Option<i32>, String)> =
            call(self.id, "total_supply_pb", protobuf, TotalSupplyArgs {})
                .await
                .map(tokens_from_proto);

        result.map_err(|(code, msg)| {
            NervousSystemError::new_with_message(
                format!(
                    "Error calling method 'total_supply' of the ledger canister. Code: {:?}. Message: {}",
                    code, msg
                )
            )
        })
    }

    async fn account_balance(
        &self,
        account: AccountIdentifier,
    ) -> Result<Tokens, NervousSystemError> {
        tla_log_request!(
            "WaitForBalanceQuery",
            Destination::new("ledger"),
            "AccountBalance",
            tla::TlaValue::Record(BTreeMap::from([
                ("account_id".to_string(), account_to_tla(account))
            ]))
        );

        let result: Result<Tokens, (Option<i32>, String)> = call(
            self.id,
            "account_balance_pb",
            protobuf,
            AccountBalanceArgs { account },
        )
        .await
        .map(tokens_from_proto);

        tla_log_response!(
            Destination::new("ledger"),
            if let Ok(balance) = result {
                tla::TlaValue::Variant {
                    tag: "BalanceQueryOk".to_string(),
                    value: Box::new(balance.get_e8s().to_tla_value()),
                }
            } else {
                tla::TlaValue::Variant {
                    tag: "Fail".to_string(),
                    value: Box::new(tla::TlaValue::Constant("UNIT".to_string())),
                }
            }
        );

        result.map_err(|(code, msg)| {
            NervousSystemError::new_with_message(
                format!(
                    "Error calling method 'account_balance_pb' of the ledger canister. Code: {:?}. Message: {}",
                    code, msg
                )
            )
        })
    }

    fn canister_id(&self) -> CanisterId {
        self.id
    }
}

/// Computes the bytes of the subaccount to which neuron staking transfers are made. This
/// function must be kept in sync with the Nervous System UI equivalent.
pub fn compute_neuron_staking_subaccount_bytes(controller: PrincipalId, nonce: u64) -> [u8; 32] {
    // The equivalent function in the NNS UI is
    // https://github.com/dfinity/dfinity_wallet/blob/351e07d3e6d007b090117161a94ce8ec9d5a6b49/js-agent/src/canisters/createNeuron.ts#L63
    const DOMAIN: &[u8] = b"neuron-stake";
    const DOMAIN_LENGTH: [u8; 1] = [0x0c];

    let mut hasher = Sha256::new();
    hasher.write(&DOMAIN_LENGTH);
    hasher.write(DOMAIN);
    hasher.write(controller.as_slice());
    hasher.write(&nonce.to_be_bytes());
    hasher.finish()
}

/// Computes the subaccount to which neuron staking transfers are made. This
/// function must be kept in sync with the Nervous System UI equivalent.
pub fn compute_neuron_staking_subaccount(controller: PrincipalId, nonce: u64) -> IcpSubaccount {
    // The equivalent function in the NNS UI is
    // https://github.com/dfinity/dfinity_wallet/blob/351e07d3e6d007b090117161a94ce8ec9d5a6b49/js-agent/src/canisters/createNeuron.ts#L63
    IcpSubaccount(compute_neuron_staking_subaccount_bytes(controller, nonce))
}

/// Computes the subaccount to which locked token distributions are initialized to.
pub fn compute_distribution_subaccount_bytes(principal_id: PrincipalId, nonce: u64) -> [u8; 32] {
    const DOMAIN: &[u8] = b"token-distribution";
    const DOMAIN_LENGTH: [u8; 1] = [0x12];

    let mut hasher = Sha256::new();
    hasher.write(&DOMAIN_LENGTH);
    hasher.write(DOMAIN);
    hasher.write(principal_id.as_slice());
    hasher.write(&nonce.to_be_bytes());
    hasher.finish()
}

/// Computes the subaccount to which locked token distributions are initialized to.
pub fn compute_distribution_subaccount(principal_id: PrincipalId, nonce: u64) -> IcpSubaccount {
    IcpSubaccount(compute_distribution_subaccount_bytes(principal_id, nonce))
}
