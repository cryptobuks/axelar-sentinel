use std::thread;

use cosmos_sdk_proto::cosmos::tx::v1beta1::{TxRaw, SimulateRequest, SimulateResponse, service_client::ServiceClient};

use error_stack::{IntoReport, Report, Result, ResultExt, IntoReportCompat};
use cosmrs::tx::{BodyBuilder, SignerInfo, Fee, SignDoc, Raw};
use cosmrs::crypto::{PublicKey,secp256k1::SigningKey};
use cosmrs::{Coin};
use tendermint::chain::Id;

use crate::tm_client::{TmClient};

use tendermint::Hash;

use crate::broadcaster::{BroadcastOptions, BroadcasterError, BroadcasterError::*};

pub fn generate_sim_tx<M>(msgs: M, sequence: u64, pub_key: &PublicKey) -> Result<Vec<u8>,BroadcasterError>
where M: IntoIterator<Item = cosmrs::Any>,
{

    let body = BodyBuilder::new().msgs(msgs).finish();
    let fee = Fee::from_amount_and_gas(Coin::new(0u8.into(), "").unwrap(), 0u64);
    let auth_info = SignerInfo::single_direct(Some(pub_key.clone()), sequence).auth_info(fee);
    let body_bytes = body.clone().into_bytes().into_report().change_context(TxMarshalingFailed)?;
    let auth_info_bytes = auth_info.clone().into_bytes().into_report().change_context(TxMarshalingFailed)?;

    let raw: Raw = TxRaw{
        body_bytes,
        auth_info_bytes,
        signatures: vec![[].to_vec()],
    }.into();

    raw.to_bytes().into_report().change_context(TxMarshalingFailed)
}

pub fn generate_tx<M>(msgs: M, priv_key: &SigningKey, account_number: u64, sequence: u64, fee: Fee, chain_id: Id) -> Result<Vec<u8>,BroadcasterError>
where M: IntoIterator<Item = cosmrs::Any>,
{
    let pub_key = priv_key.public_key();
    let auth_info = SignerInfo::single_direct(Some(pub_key), sequence).auth_info(fee);
    let body = BodyBuilder::new().msgs(msgs).finish();
            
    SignDoc::new(&body, &auth_info, &chain_id, account_number)
        .and_then(|sign_doc| sign_doc.sign(&priv_key))
        .and_then(|tx| tx.to_bytes()).into_report().change_context(TxMarshalingFailed)
}


pub async fn wait_for_block_inclusion<C>(tm_client: C, tx_hash:  Hash, options:  BroadcastOptions) -> Result<(),BroadcasterError>
where C: TmClient + Clone,
{
    let mut last_error = Report::new(BlockInclusionTimeout);

    for _ in 0..options.tx_fetch_max_retries {
        thread::sleep(options.tx_fetch_interval);
        match tm_client.clone().get_tx(tx_hash.clone(), true).await {
            Ok(_) => return Ok(()),
            Err(err) => {
                last_error = err.change_context(BlockInclusionTimeout);
                continue;
            }
        }
    }

    Err(last_error)
}

pub async fn simulate(grpc_url: String, tx_bytes: Vec<u8>) -> Result<SimulateResponse,BroadcasterError> {
        let request = SimulateRequest{
            tx: None,
            tx_bytes,
        };

        let mut client = ServiceClient::connect(grpc_url)
            .await.into_report().change_context(ConnectionFailed)?;

        match client.simulate(request).await {
            Ok(response) => Ok(response.into_inner()),
            Err(err) => {
                if err.code() == 32.into() {
                    return Err(err).into_report().change_context(AccountSequenceMismatch)
                } 
                Err(err).into_report().change_context(TxSimulationFailed)
            },
        }
    }

#[cfg(test)]
mod tests {

    use cosmrs::tx::{Fee, Msg};
    use cosmrs::bank::MsgSend;
    use cosmrs::crypto::secp256k1::{SigningKey};
    use cosmrs::tx::AccountNumber;
    use cosmrs::Coin;
    use std::str;
    use std::iter;

    use super::{generate_tx, generate_sim_tx};

    const CHAIN_ID: &str = "axelar-unit-test";
    const ACC_NUMBER: AccountNumber = 1;
    const ACC_PREFIX: &str = "axelar";
    const DENOM: &str = "uaxl";
    const PRIV_CONST_KEY: &str = "661fdf5983a27f9ecff7bbc383393cf8bd305b477ade940f83fd22f8e35d6c21";


    #[test]
    fn marshal_sim_tx_success() {
        let mut priv_key_bytes = [0; PRIV_CONST_KEY.len() / 2];
        hex::decode_to_slice(PRIV_CONST_KEY, &mut priv_key_bytes).expect("Decoding failed");
        let account_id = SigningKey::from_bytes(&priv_key_bytes).expect("panic!").public_key().account_id(ACC_PREFIX).unwrap();
    
        let recipient_private_key = SigningKey::random();
        let recipient_account_id = recipient_private_key
            .public_key()
            .account_id(ACC_PREFIX)
            .unwrap();
    
            
        let amount = Coin {
            amount: 1u8.into(),
            denom: DENOM.parse().unwrap(),
        };
    
        let msg_send = MsgSend {
            from_address: account_id.clone(),
            to_address: recipient_account_id,
            amount: vec![amount.clone()],
        }
        .to_any()
        .unwrap();
      

        let res = generate_sim_tx(
            iter::once(msg_send),
            0,
            &SigningKey::from_bytes(&priv_key_bytes).expect("panic!").public_key()
        );

        assert!(res.is_ok());

    }


    #[test]
    fn marshal_tx_success() {
        let mut priv_key_bytes = [0; PRIV_CONST_KEY.len() / 2];
        hex::decode_to_slice(PRIV_CONST_KEY, &mut priv_key_bytes).expect("Decoding failed");
        let priv_key = SigningKey::from_bytes(&priv_key_bytes).expect("panic!");
        let account_id = priv_key.public_key().account_id(ACC_PREFIX).unwrap();
    
        let recipient_private_key = SigningKey::random();
        let recipient_account_id = recipient_private_key
            .public_key()
            .account_id(ACC_PREFIX)
            .unwrap();

        let amount = Coin {
            amount: 1u8.into(),
            denom: DENOM.parse().unwrap(),
        };
    
        let msg_send = MsgSend {
            from_address: account_id.clone(),
            to_address: recipient_account_id,
            amount: vec![amount.clone()],
        }
        .to_any()
        .unwrap();
    
        let chain_id = CHAIN_ID.parse().unwrap();
        let seq_number = 0;
        let gas = 100_000u64;
        let fee = Fee::from_amount_and_gas(amount, gas);

        let res = generate_tx(
            iter::once(msg_send),
            &priv_key,
            ACC_NUMBER,
            seq_number,
            fee,
            chain_id
        );        

        assert!(res.is_ok());

    }

   
}
