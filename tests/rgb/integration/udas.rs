#![cfg(not(target_arch = "wasm32"))]
use crate::rgb::integration::utils::{
    create_new_invoice, create_new_psbt, create_new_transfer, get_real_uda_data, get_uda_data,
    issuer_issue_contract_v2, UtxoFilter, ISSUER_MNEMONIC, OWNER_MNEMONIC,
};
use bitmask_core::{
    bitcoin::{save_mnemonic, sign_and_publish_psbt_file},
    rgb::{accept_transfer, import_uda_data, structs::ContractAmount},
    structs::{AcceptRequest, IssueMediaRequest, SecretString, SignPsbtRequest},
};

#[tokio::test]
async fn accept_uda_transfer() -> anyhow::Result<()> {
    let issuer_keys = &save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let owner_keys = save_mnemonic(
        &SecretString(OWNER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let meta = Some(get_uda_data());
    let issuer_resp = issuer_issue_contract_v2(
        1,
        "RGB21",
        ContractAmount::new(1, 0).to_value(),
        false,
        true,
        meta,
        Some("0.1".to_string()),
        Some(UtxoFilter::with_amount_equal_than(10_000_000)),
        None,
    )
    .await?;
    let issuer_resp = issuer_resp[0].clone();
    let owner_resp = create_new_invoice(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        1.0,
        owner_keys,
        None,
        Some(issuer_resp.clone().contract.legacy),
    )
    .await?;
    let psbt_resp = create_new_psbt(
        &issuer_resp.contract_id,
        &issuer_resp.iface,
        vec![issuer_resp.issue_utxo.clone()],
        issuer_keys.clone(),
    )
    .await?;
    let transfer_resp = create_new_transfer(issuer_keys.clone(), owner_resp, psbt_resp).await?;

    let sk = issuer_keys.private.nostr_prv.to_string();
    let request = SignPsbtRequest {
        psbt: transfer_resp.psbt,
        descriptors: [SecretString(
            issuer_keys.private.rgb_udas_descriptor_xprv.clone(),
        )]
        .to_vec(),
    };
    let resp = sign_and_publish_psbt_file(request).await;
    assert!(resp.is_ok());

    let request = AcceptRequest {
        consignment: transfer_resp.consig,
        force: false,
    };

    let resp = accept_transfer(&sk, request).await;
    assert!(resp.is_ok());
    assert!(resp?.valid);

    Ok(())
}

#[tokio::test]
async fn create_uda_save_medias() -> anyhow::Result<()> {
    let issuer_keys = &save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;

    let metadata = get_real_uda_data();
    let resp = import_uda_data(metadata).await?;
    let meta = Some(IssueMediaRequest::from(resp));

    let _issuer_resp = issuer_issue_contract_v2(
        1,
        "RGB21",
        ContractAmount::new(1, 0).to_value(),
        false,
        true,
        meta,
        Some("0.1".to_string()),
        Some(UtxoFilter::with_amount_equal_than(10_000_000)),
        Some(issuer_keys.clone()),
    )
    .await?;
    Ok(())
}
