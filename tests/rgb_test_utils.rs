use std::{convert::Infallible, str::FromStr};

use amplify::hex::{FromHex, ToHex};
use bitcoin::Transaction;
use bitmask_core::{
    operations::rgb::issue::issue_contract,
    operations::rgb::{invoice::create_invoice, schemas::default_fungible_iimpl},
};
use bp::{Outpoint, Sats, ScriptPubkey, Tx, TxIn, TxOut, TxVer, Txid, VarIntArray, Vout};
use psbt::{serialize::Deserialize, Psbt};
use rgbstd::{
    containers::BindleContent,
    contract::{ContractId, GraphSeal},
    interface::rgb20,
    persistence::{Inventory, Stock},
    resolvers::ResolveHeight,
    validation::ResolveTx as RgbResolveTx,
};
use rgbwallet::RgbInvoice;
use wallet::onchain::ResolveTx;

// Resolvers
pub struct DumbResolve {}

impl ResolveHeight for DumbResolve {
    type Error = Infallible;
    fn resolve_height(&mut self, _txid: Txid) -> std::result::Result<u32, Self::Error> {
        Ok(0)
    }
}
impl ResolveTx for DumbResolve {
    fn resolve_tx(
        &self,
        _txid: bitcoin::Txid,
    ) -> Result<Transaction, wallet::onchain::TxResolverError> {
        let hex = "020000000001014fba153e23558ca5532b5187ac20c4e35fe588c9bcb4a7b3c881c0541fcda65c0100000000ffffffff0118ddf50500000000225120d9b9957aa15bb91d856ed862cd04183555c9b9ea04ec3763c3b1e388adebe8e601417b5df1ce9c9c56c914203d8b2827000c72a15733e85f18c6a35f1fafa9c5068a8c73169dc3d98113112d7309114ca449fe3f740e949dbc6712ff945115d666c10100000000";
        let transaction = Transaction::deserialize(&Vec::from_hex(hex).unwrap()).unwrap();
        Ok(transaction)
    }
}

impl RgbResolveTx for DumbResolve {
    fn resolve_tx(&self, _txid: Txid) -> Result<bp::Tx, rgbstd::validation::TxResolverError> {
        let hex = "020000000001014fba153e23558ca5532b5187ac20c4e35fe588c9bcb4a7b3c881c0541fcda65c0100000000ffffffff0118ddf50500000000225120d9b9957aa15bb91d856ed862cd04183555c9b9ea04ec3763c3b1e388adebe8e601417b5df1ce9c9c56c914203d8b2827000c72a15733e85f18c6a35f1fafa9c5068a8c73169dc3d98113112d7309114ca449fe3f740e949dbc6712ff945115d666c10100000000";
        let transaction = Transaction::deserialize(&Vec::from_hex(hex).unwrap()).unwrap();

        let mut ti = VarIntArray::new();
        let tx_input = &transaction.input[0];
        let prevout = &transaction.input[0].previous_output;
        let input = TxIn {
            prev_output: bp::Outpoint {
                txid: bp::Txid::from_hex(&prevout.txid.to_hex()).expect("fail"),
                vout: bp::Vout::from(prevout.vout),
            },
            sequence: bp::SeqNo::from(tx_input.sequence.0),
            sig_script: bp::SigScript::default(),
        };
        ti.push(input).expect("fail");

        let mut to = VarIntArray::new();
        let tx_output = &transaction.output[0];
        let output = TxOut {
            value: Sats::from(tx_output.value),
            script_pubkey: ScriptPubkey::from(tx_output.script_pubkey.to_bytes()),
        };
        to.push(output).expect("fail");

        let tx = Tx {
            version: TxVer::V2,
            inputs: ti,
            outputs: to,
            lock_time: 422.into(),
        };
        Ok(tx)
    }
}

// Helpers
#[allow(dead_code)]
pub fn dumb_psbt() -> Psbt {
    let psbt_hex = "70736274ff01005e02000000014fba153e23558ca5532b5187ac20c4e35fe588c9bcb4a7b3c881c0541fcda65c0100000000ffffffff0118ddf505000000002251202aa594ee4dc05d289387c77a44ee3d5401a7edc269e355f2345c2792d9f8d014000000004f01043587cf034a3acf0b80000000fe80c9c11d65f2a2bfbf8e582c49b829e0f453e2a7138ec303ddd724aa295ebf02008b0bc2899bf59a892479c553d7c7e6901a0fc8db3e5570529101bc783743bf10280a59635600008001000080000000800001008902000000019d8420cc5666b02f260bbaea43326c50a2c2eb99292fcf4c42a6179e132344de0000000000fdffffff02db9a8b44000000002251205d853a4a3da1dc163d2a2d9e8a76ae63db83f9310a25caa5d216a0fd962923a900e1f505000000002251206a61bf8aea7388b8541f16d773b77f897110eaa6bc17ada61c50bc70a93e5d61f4010000010304010000002116e7e50584e394cb1b467f440e8760bf3806835d55378f78cbacb8c651d2e11d0f1900280a59635600008001000080000000800000000000000000011720e7e50584e394cb1b467f440e8760bf3806835d55378f78cbacb8c651d2e11d0f0022020269c3a787c625331a17fd8a5cf7094d4672fb0385b5fd8fa2813181de3a1cef3e18280a5963560000800100008000000080010000000000000001052069c3a787c625331a17fd8a5cf7094d4672fb0385b5fd8fa2813181de3a1cef3e09fc06544150524554000000";
    Psbt::from_str(psbt_hex).expect("invalid dumb psbt")
}

#[allow(dead_code)]
pub fn generate_new_contract(mut stock: Stock) -> (ContractId, Stock) {
    let ticker = "DIBA1";
    let name = "DIBA1";
    let description =
        "1 2 3 testing... 1 2 3 testing... 1 2 3 testing... 1 2 3 testing.... 1 2 3 testing";
    let precision = 8;
    let supply = 10;
    let seal = "tapret1st:5ca6cd1f54c081c8b3a7b4bcc988e55fe3c420ac87512b53a58c55233e15ba4f:1";

    let iface = rgb20();
    let iimpl = default_fungible_iimpl();

    let contract = issue_contract(
        ticker,
        name,
        description,
        precision,
        supply,
        seal,
        iface,
        iimpl,
    )
    .expect("test issue_contract failed");

    let mut dumb = DumbResolve {};

    let bindle = contract.bindle();
    let contract = bindle
        .unbindle()
        .validate(&mut dumb)
        .map_err(|c| c.validation_status().expect("just validated").to_string())
        .expect("invalid contract");

    stock
        .import_contract(contract.clone(), &mut dumb)
        .expect("import_contract failed");
    (contract.contract_id(), stock)
}

#[allow(dead_code)]
pub fn generate_new_invoice(
    contract_id: ContractId,
    stock: Stock,
    txid: String,
    vout: u32,
) -> RgbInvoice {
    let amount = 1;
    let iface = rgb20();
    let txid: Txid = txid.parse().expect("invalid txid");
    let seal = GraphSeal::tapret_first(txid, vout);
    create_invoice(contract_id, iface, amount, seal, stock).expect("create_invoice failed")
}
