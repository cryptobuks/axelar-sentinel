
use std::time::Duration;

use error_stack::{Result, ResultExt};
use thiserror::Error;

use cosmrs::tx::Fee;
use cosmrs::crypto::secp256k1::SigningKey;
use cosmrs::Denom;
use tendermint::chain::Id;

use crate::broadcaster::BroadcasterError::*;
use crate::tm_client::{TmClient, BroadcastResponse};
use crate::broadcaster::account_client::GasEstimator;

pub mod account_client;
mod helpers;

#[derive(Error, Debug)]
pub enum BroadcasterError {
    #[error("failed to connect to node")]
    ConnectionFailed,
    #[error("tx marshaling failed")]
    TxMarshalingFailed,
    #[error("timeout for tx inclusion in block")]
    BlockInclusionTimeout,
    #[error("failed to estimate gas")]
    GasEstimationFailed,
}

#[derive(Clone)]
pub struct BroadcastOptions {
    pub tx_fetch_interval: Duration,
    pub tx_fetch_max_retries: u32,
    pub gas_adjustment: f32,
    pub gas_price: (f64, Denom),
}

pub struct Broadcaster<T: TmClient, G: GasEstimator> {
    tm_client: T,
    gas_estimator: G,
    acc_number: u64,
    acc_sequence: u64,
    chain_id: Id,
    priv_key: SigningKey,
    options: BroadcastOptions,
}

impl<T: TmClient, G: GasEstimator> Broadcaster<T,G> {
    pub fn new(tm_client: T, gas_estimator: G, acc_number: u64, acc_sequence: u64, options: BroadcastOptions, priv_key: SigningKey, chain_id: Id) -> Self {
        Broadcaster { tm_client, gas_estimator, acc_number, acc_sequence, options, priv_key, chain_id}
    }

    pub async fn broadcast<M>(&mut self, msgs: M) -> Result<BroadcastResponse,BroadcasterError>
    where M: IntoIterator<Item = cosmrs::Any> + Clone,
    {
        let tx_bytes = helpers::generate_sim_tx(msgs.clone(), self.acc_sequence, &self.priv_key.public_key())?;
        let estimated_gas = self.gas_estimator.estimate_gas(tx_bytes).await.change_context(GasEstimationFailed)?;
        let mut gas_limit = estimated_gas;
        if self.options.gas_adjustment > 0.0 {
            gas_limit = (gas_limit as f64 * self.options.gas_adjustment as f64) as u64;
        }

        let (value,denom) = self.options.gas_price.clone();
        let amount = cosmrs::Coin{
            amount:  (gas_limit as f64 * value).ceil() as u128,
            denom: denom,
        };

        let fee = Fee::from_amount_and_gas(amount, gas_limit);
        let tx_bytes= helpers::generate_tx(
            msgs,
            &self.priv_key,
            self.acc_number,
            self.acc_sequence,
            fee,
            self.chain_id.clone(),
        )?;
        let response = self.tm_client.broadcast(tx_bytes).await.change_context(ConnectionFailed)?;

        helpers::wait_for_block_inclusion(
            &self.tm_client,
            response.hash,
            self.options.tx_fetch_interval,
            self.options.tx_fetch_max_retries,
        ).await?;

        self.acc_sequence += 1;
        Ok(response)

    }
}
