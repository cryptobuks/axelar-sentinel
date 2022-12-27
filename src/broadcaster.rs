
use std::time::{Duration};

use error_stack::{Result, ResultExt};
use thiserror::Error;

use cosmrs::tx::Fee;
use cosmrs::crypto::secp256k1::SigningKey;
use cosmrs::Denom;
use tendermint::chain::Id;

use crate::broadcaster::BroadcasterError::*;
use crate::tm_client::{TmClient, BroadcastResponse};
use crate::ClientContext;

pub mod client_context;
mod helpers;

#[derive(Error, Debug)]
pub enum BroadcasterError {
    #[error("failed to connect to node")]
    ConnectionFailed,
    #[error("tx marshaling failed")]
    TxMarshalingFailed,
    #[error("tx simulation failed")]
    TxSimulationFailed,
    #[error("account sequence mismatch during simulation")]
    AccountSequenceMismatch,
    #[error("timeout for tx inclusion in block")]
    BlockInclusionTimeout,
    #[error("failed to update local context")]
    ContextUpdateFailed,
    #[error("failed to estimate gas")]
    GasEstimationFailed,
}

#[derive(Clone, Copy)]
pub struct BroadcastOptions {
    pub tx_fetch_interval: Duration,
    pub tx_fetch_max_retries: u32,
    pub gas_adjustment: f32,
}

pub struct Broadcaster<T: TmClient + Clone, C: ClientContext> {
    node: T,
    client_context: C,
    priv_key: SigningKey,
    options: BroadcastOptions,
    gas_price: (f64, Denom),
    chain_id: Option<Id>,
}

impl<T: TmClient + Clone, C: ClientContext> Broadcaster<T,C> {
    pub fn new(node: T, context: C, options: BroadcastOptions, priv_key: SigningKey, gas_price: (f64,Denom)) -> Self {
        Broadcaster { node, options, client_context: context, priv_key, gas_price, chain_id: None}
    }

    pub async fn broadcast<M>(&mut self, msgs: M) -> Result<BroadcastResponse,BroadcasterError>
    where M: IntoIterator<Item = cosmrs::Any> + Clone,
    {
        if self.client_context.sequence().is_none() || self.client_context.account_number().is_none() {
            self.client_context.update_account_info().await.change_context(ContextUpdateFailed)?;
        }

        if self.chain_id.is_none() {
            self.chain_id = Some(self.node.clone().get_status().await.change_context(ContextUpdateFailed)?.node_info.network);
        }

        let sequence = self.client_context.sequence().ok_or(ContextUpdateFailed)?;
        let account_number = self.client_context.account_number().ok_or(ContextUpdateFailed)?;
        let chain_id = self.chain_id.clone().ok_or(ContextUpdateFailed)?;
        let gas_adjustment = self.options.gas_adjustment;

        let tx_bytes = helpers::generate_sim_tx(msgs.clone(), sequence, &self.priv_key.public_key())?;
        let estimated_gas = self.client_context.estimate_gas(tx_bytes).await.change_context(GasEstimationFailed)?;
        
        let mut gas_limit = estimated_gas;
        if self.options.gas_adjustment > 0.0 {
            gas_limit = (gas_limit as f64 * gas_adjustment as f64) as u64;
        }

        let (value,denom) = self.gas_price.clone();
        let amount = cosmrs::Coin{
            amount:  (gas_limit as f64 * value).ceil() as u128,
            denom: denom,
        };

        let fee = Fee::from_amount_and_gas(amount, gas_limit);
        let tx_bytes= helpers::generate_tx(msgs.clone(), &self.priv_key, account_number, sequence, fee, chain_id)?;
        let response = self.node.broadcast(tx_bytes).await.change_context(ConnectionFailed)?;

        helpers::wait_for_block_inclusion(self.node.clone(), response.hash, self.options).await?;

        Ok(response)

    }
}
