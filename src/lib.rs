use error_stack::Result;
use tendermint_rpc::{SubscriptionClient, WebSocketClient};

use crate::report::Error;
use crate::broadcaster::*;
use crate::broadcaster::client_context::*;

use cosmrs::crypto::secp256k1::SigningKey;
use cosmrs::Coin;
use cosmrs::bank::MsgSend;
use cosmrs::tx::Msg;

pub mod config;
pub mod event_sub;
pub mod report;
pub mod tm_client;
pub mod broadcaster;

pub async fn run(_cfg: config::Config) -> Result<(), Error> {

    let grpc_url = "tcp://af393a310eb1a4ec4883c83f6fc4523b-917064975.us-east-2.elb.amazonaws.com:9090";
    let tm_url = "ws://af393a310eb1a4ec4883c83f6fc4523b-917064975.us-east-2.elb.amazonaws.com:26657/websocket";

    //test account
    let address = "axelar1y7zhht60mr392ffkhpdj5tunlcap2rhx3tsnqw";
    let mut cc = GrpcClientContext::new(String::from(address), String::from(grpc_url));
    cc.update_account_info().await.expect("failed to fetch account info");
    let sequence = cc.sequence().unwrap();
    println!("{} current sequence number is {:?}", address, sequence);

    let (tm_client, tm_driver) = WebSocketClient::new(tm_url).await.expect("error creating http client");
    let driver_handle = tokio::spawn(async move { tm_driver.run().await });
    
    const PRIV_CONST_KEY: &str = "661fdf5983a27f9ecff7bbc383393cf8bd305b477ade940f83fd22f8e35d6c21";
    let mut priv_key_bytes = [0; PRIV_CONST_KEY.len() / 2];
    hex::decode_to_slice(PRIV_CONST_KEY, &mut priv_key_bytes).expect("Decoding failed");
    let priv_key = SigningKey::from_bytes(&priv_key_bytes).expect("panic!");

    let options = broadcaster::BroadcastOptions{
        tx_fetch_interval: std::time::Duration::new(5, 0),
        tx_fetch_max_retries: 10,
        gas_adjustment: 1.5,
    };

    let account_id = priv_key.public_key().account_id("axelar").unwrap();

    let recipient_private_key = SigningKey::random();
    let recipient_account_id = recipient_private_key
        .public_key()
        .account_id("axelar")
        .unwrap();

    let amount = Coin {
        amount: 1u8.into(),
        denom: "ujcs".parse().unwrap(),
    };

    let msg_send = MsgSend {
        from_address: account_id.clone(),
        to_address: recipient_account_id,
        amount: vec![amount.clone()],
    }
    .to_any()
    .unwrap();

    let mut broadcaster = broadcaster::Broadcaster::new(tm_client.clone(), cc, options, priv_key, (0.00005,"ujcs".parse().unwrap()));
    let response = broadcaster.broadcast(std::iter::once(msg_send)).await.expect("failed to broadcast!");
    println!("{:?}",response);

    tm_client.close().unwrap();
    let _ = driver_handle.await.unwrap();

    Ok(())
}
