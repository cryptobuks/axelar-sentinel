use async_trait::async_trait;

use cosmos_sdk_proto::traits::Message;
use cosmos_sdk_proto::cosmos::auth::v1beta1::query_client::{QueryClient as AuthQueryClient};
use cosmos_sdk_proto::cosmos::auth::v1beta1::{QueryAccountRequest, BaseAccount};
use error_stack::{IntoReport, ResultExt, Result};
use thiserror::Error;

use crate::client_context::ClientContextError::*;
use crate::broadcaster::helpers::simulate;

#[derive(Error, Debug)]
pub enum ClientContextError {
    #[error("failed to connect to node")]
    ConnectionFailed,
    #[error("remote call failed")]
    RemoteCallFailed,
    #[error("account sequence mismatch during simulation")]
    AccountSequenceMismatch,
    #[error("failed to unmarshal protobuf")]
    UnmarshalingFailed,
    #[error("tx simulation failed")]
    TxSimulationFailed,
}

#[async_trait]
pub trait ClientContext {
    fn sequence(&self) -> Option<u64>;
    fn account_number(&self) -> Option<u64>;
    async fn update_account_info(&mut self) -> Result<(),ClientContextError>;
    async fn estimate_gas(&self, tx_bytes: Vec<u8>) -> Result<u64,ClientContextError>;
}


pub struct GrpcClientContext{
    grpc_url: String,
    address: String,
    account_info: Option<BaseAccount>,
}


impl GrpcClientContext {
    pub fn new(address: String, grpc_url: String) -> impl ClientContext {
        GrpcClientContext{
            grpc_url: grpc_url.clone(),
            address: address.clone(),
            account_info: None}
    }
}

#[async_trait]
impl ClientContext for GrpcClientContext {
    fn sequence(&self) -> Option<u64> {
       self.account_info.clone().map(|info | info.sequence)
    }

    fn account_number(&self) -> Option<u64> {
        self.account_info.clone().map(|info | info.account_number)
    }

    async fn update_account_info(&mut self) -> Result<(),ClientContextError> {

        let request = QueryAccountRequest{address: self.address.clone()};

        let mut client = AuthQueryClient::connect(self.grpc_url.clone())
            .await.into_report().change_context(ConnectionFailed)?;

        let response = client.account(request)
            .await.into_report().change_context(RemoteCallFailed)?;

        let account = response.into_inner().account
            .ok_or(UnmarshalingFailed).into_report().and_then(| account | {
                BaseAccount::decode(&account.value[..])
                .into_report().change_context(UnmarshalingFailed)
            })?;

        self.account_info = Some(account);
        Ok(())

    }

    async fn estimate_gas(&self, tx_bytes: Vec<u8>) -> Result<u64,ClientContextError> {
        simulate(self.grpc_url.clone(), tx_bytes).await?
        .gas_info.ok_or(TxSimulationFailed)
        .map(| info | info.gas_used).into_report()
    }
}
