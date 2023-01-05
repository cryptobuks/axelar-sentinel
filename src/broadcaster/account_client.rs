use async_trait::async_trait;

use cosmos_sdk_proto::traits::Message;
use cosmos_sdk_proto::cosmos::auth::v1beta1::query_client::{QueryClient as AuthQueryClient};
use cosmos_sdk_proto::cosmos::auth::v1beta1::{QueryAccountRequest, BaseAccount};
use error_stack::{IntoReport, ResultExt, Result};
use thiserror::Error;

use cosmos_sdk_proto::cosmos::tx::v1beta1::{SimulateRequest, service_client::ServiceClient};

const SEQ_MISMATCH_CODE: i32 = 32;

#[derive(Error, Debug)]
pub enum AccountInfoError {
    #[error("failed to connect to node")]
    ConnectionFailed,
    #[error("remote call failed")]
    RemoteCallFailed,
    #[error("failed to unmarshal protobuf")]
    UnmarshalingFailed,
}

#[derive(Error, Debug)]
pub enum GasEstimatorError {
    #[error("failed to connect to node")]
    ConnectionFailed,
    #[error("remote call failed")]
    RemoteCallFailed,
    #[error("account sequence mismatch during simulation")]
    AccountSequenceMismatch,
    #[error("tx simulation failed")]
    TxSimulationFailed,
}

#[async_trait]
pub trait AccountInfo {
    fn sequence(&self) -> Option<u64>;
    fn account_number(&self) -> Option<u64>;
    async fn synch(&mut self) -> Result<(),AccountInfoError>;
}

#[async_trait]
pub trait GasEstimator {
    async fn estimate_gas(&self, tx_bytes: Vec<u8>) -> Result<u64,GasEstimatorError>;
}


pub struct GrpcAccountClient{
    grpc_url: String,
    address: String,
    account_info: Option<BaseAccount>,
}

impl GrpcAccountClient{
    pub fn new(address: String, grpc_url: String) -> Self {
        GrpcAccountClient{
            grpc_url: grpc_url,
            address: address,
            account_info: None}
    }
}

#[async_trait]
impl AccountInfo for GrpcAccountClient {
    fn sequence(&self) -> Option<u64> {
       self.account_info.clone().map(|info | info.sequence)
    }

    fn account_number(&self) -> Option<u64> {
        self.account_info.clone().map(|info | info.account_number)
    }

    async fn synch(&mut self) -> Result<(),AccountInfoError> {
        if self.account_info.is_some() {
            return Ok(());
        }

        let request = QueryAccountRequest{address: self.address.clone()};

        let mut client = AuthQueryClient::connect(self.grpc_url.clone())
            .await.into_report().change_context(AccountInfoError::ConnectionFailed)?;

        let response = client.account(request)
            .await.into_report().change_context(AccountInfoError::RemoteCallFailed)?;

        let account = response.into_inner().account
            .ok_or(AccountInfoError::UnmarshalingFailed).into_report().and_then(| account | {
                BaseAccount::decode(&account.value[..])
                .into_report().change_context(AccountInfoError::UnmarshalingFailed)
            })?;

        self.account_info = Some(account);
        Ok(())

    }
}

#[async_trait]
impl GasEstimator for GrpcAccountClient {
    async fn estimate_gas(&self, tx_bytes: Vec<u8>) -> Result<u64,GasEstimatorError> {
        let request = SimulateRequest{
            tx: None,
            tx_bytes,
        };

        let mut client = ServiceClient::connect(self.grpc_url.clone())
            .await.into_report().change_context(GasEstimatorError::ConnectionFailed)?;

        match client.simulate(request).await {
            Ok(response) => response
                .into_inner()
                .gas_info
                .ok_or(GasEstimatorError::TxSimulationFailed)
                .map(| info | info.gas_used)
                .into_report(),
            Err(err) => {
                if err.code() == SEQ_MISMATCH_CODE.into() {
                    return Err(err).into_report().change_context(GasEstimatorError::AccountSequenceMismatch)
                } 
                Err(err).into_report().change_context(GasEstimatorError::TxSimulationFailed)
            },
        }
    }
}
