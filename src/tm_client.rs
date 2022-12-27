use async_trait::async_trait;
use error_stack::{Report, Result};
use futures::TryFutureExt;

use tendermint::Hash;
use tendermint::block::Height;
use tendermint_rpc::{Client, Subscription, SubscriptionClient, WebSocketClient};

use tokio_stream::Stream;

pub type BlockResponse = tendermint_rpc::endpoint::block_results::Response;
pub type BroadcastResponse = tendermint_rpc::endpoint::broadcast::tx_sync::Response;
pub type TxResponse = tendermint_rpc::endpoint::tx::Response;
pub type StatusResponse = tendermint_rpc::endpoint::status::Response;

pub type Error = tendermint_rpc::Error;
pub type Query = tendermint_rpc::query::Query;
pub type Event = tendermint_rpc::event::Event;
pub type EventData = tendermint_rpc::event::EventData;
pub type EventType = tendermint_rpc::query::EventType;

#[async_trait]
pub trait TmClient {
    type Sub: Stream<Item = core::result::Result<Event, Error>> + Unpin;

    async fn subscribe(&self, query: Query) -> Result<Self::Sub, Error>;
    async fn block_results(&self, block_height: Height) -> Result<BlockResponse, Error>;
    async fn broadcast(&self, tx_raw: Vec<u8>) -> Result<BroadcastResponse, Error>;
    async fn get_tx(self, tx_hash: Hash, prove: bool) -> Result<TxResponse,Error>;
    async fn get_status(self) -> Result<StatusResponse,Error>;
    fn close(self) -> Result<(), Error>;
}

#[async_trait]
impl TmClient for WebSocketClient {
    type Sub = Subscription;

    async fn subscribe(&self, query: Query) -> Result<Self::Sub, Error> {
        SubscriptionClient::subscribe(self, query).map_err(Report::new).await
    }

    async fn block_results(&self, block_height: Height) -> Result<BlockResponse, Error> {
        Client::block_results(self, block_height).map_err(Report::new).await
    }
    async fn broadcast(&self, tx_raw: Vec<u8>) -> Result<BroadcastResponse, Error> {
        Client::broadcast_tx_sync(self, tx_raw).map_err(Report::new).await
    }
    fn close(self) -> Result<(), Error> {
        SubscriptionClient::close(self).map_err(Report::new)
    }
    async fn get_tx(self, tx_hash: Hash, prove: bool) -> Result<TxResponse,Error> {
        Client::tx(&self, tx_hash, prove).map_err(Report::new).await
    }

    async fn get_status(self) -> Result<StatusResponse,Error> {
        Client::status(&self).map_err(Report::new).await
    }
}
