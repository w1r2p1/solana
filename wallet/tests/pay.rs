use chrono::prelude::*;
use serde_json::{json, Value};
use solana::rpc_request::{RpcClient, RpcRequest, RpcRequestHandler};
use solana_drone::drone::run_local_drone;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, KeypairUtil};
use solana_wallet::wallet::{
    process_command, request_and_confirm_airdrop, WalletCommand, WalletConfig,
};
use std::fs::remove_dir_all;
use std::sync::mpsc::channel;

#[cfg(test)]
use solana::thin_client::new_fullnode;

fn check_balance(expected_balance: u64, client: &RpcClient, params: Value) {
    let balance = client
        .make_rpc_request(1, RpcRequest::GetBalance, Some(params))
        .unwrap()
        .as_u64()
        .unwrap();
    assert_eq!(balance, expected_balance);
}

#[test]
fn test_wallet_timestamp_tx() {
    let (server, leader_data, alice, ledger_path) = new_fullnode("test_wallet_timestamp_tx");
    let server_exit = server.run(None);
    let bob_pubkey = Keypair::new().pubkey();

    let (sender, receiver) = channel();
    run_local_drone(alice, sender);
    let drone_addr = receiver.recv().unwrap();

    let rpc_client = RpcClient::new_from_socket(leader_data.rpc);

    let mut config_payer = WalletConfig::default();
    config_payer.drone_port = drone_addr.port();
    config_payer.rpc_port = leader_data.rpc.port();

    let mut config_witness = WalletConfig::default();
    config_witness.drone_port = drone_addr.port();
    config_witness.rpc_port = leader_data.rpc.port();

    assert_ne!(config_payer.id.pubkey(), config_witness.id.pubkey());

    request_and_confirm_airdrop(&rpc_client, &drone_addr, &config_payer.id, 50).unwrap();
    let params = json!([format!("{}", config_payer.id.pubkey())]);
    check_balance(50, &rpc_client, params);

    // Make transaction (from config_payer to bob_pubkey) requiring timestamp from config_witness
    let date_string = "\"2018-09-19T17:30:59Z\"";
    let dt: DateTime<Utc> = serde_json::from_str(&date_string).unwrap();
    config_payer.command = WalletCommand::Pay(
        10,
        bob_pubkey,
        Some(dt),
        Some(config_witness.id.pubkey()),
        None,
        None,
    );
    let sig_response = process_command(&config_payer);

    let object: Value = serde_json::from_str(&sig_response.unwrap()).unwrap();
    let process_id_str = object.get("processId").unwrap().as_str().unwrap();
    let process_id_vec = bs58::decode(process_id_str)
        .into_vec()
        .expect("base58-encoded public key");
    let process_id = Pubkey::new(&process_id_vec);

    let params = json!([format!("{}", config_payer.id.pubkey())]);
    check_balance(39, &rpc_client, params); // config_payer balance
    let params = json!([format!("{}", process_id)]);
    check_balance(11, &rpc_client, params); // contract balance
    let params = json!([format!("{}", bob_pubkey)]);
    check_balance(0, &rpc_client, params); // recipient balance

    // Sign transaction by config_witness
    config_witness.command = WalletCommand::TimeElapsed(bob_pubkey, process_id, dt);
    process_command(&config_witness).unwrap();

    let params = json!([format!("{}", config_payer.id.pubkey())]);
    check_balance(39, &rpc_client, params); // config_payer balance
    let params = json!([format!("{}", process_id)]);
    check_balance(1, &rpc_client, params); // contract balance
    let params = json!([format!("{}", bob_pubkey)]);
    check_balance(10, &rpc_client, params); // recipient balance

    server_exit();
    remove_dir_all(ledger_path).unwrap();
}

#[test]
fn test_wallet_witness_tx() {
    let (server, leader_data, alice, ledger_path) = new_fullnode("test_wallet_witness_tx");
    let server_exit = server.run(None);
    let bob_pubkey = Keypair::new().pubkey();

    let (sender, receiver) = channel();
    run_local_drone(alice, sender);
    let drone_addr = receiver.recv().unwrap();

    let rpc_client = RpcClient::new_from_socket(leader_data.rpc);

    let mut config_payer = WalletConfig::default();
    config_payer.drone_port = drone_addr.port();
    config_payer.rpc_port = leader_data.rpc.port();

    let mut config_witness = WalletConfig::default();
    config_witness.drone_port = drone_addr.port();
    config_witness.rpc_port = leader_data.rpc.port();

    assert_ne!(config_payer.id.pubkey(), config_witness.id.pubkey());

    request_and_confirm_airdrop(&rpc_client, &drone_addr, &config_payer.id, 50).unwrap();

    // Make transaction (from config_payer to bob_pubkey) requiring witness signature from config_witness
    config_payer.command = WalletCommand::Pay(
        10,
        bob_pubkey,
        None,
        None,
        Some(vec![config_witness.id.pubkey()]),
        None,
    );
    let sig_response = process_command(&config_payer);

    let object: Value = serde_json::from_str(&sig_response.unwrap()).unwrap();
    let process_id_str = object.get("processId").unwrap().as_str().unwrap();
    let process_id_vec = bs58::decode(process_id_str)
        .into_vec()
        .expect("base58-encoded public key");
    let process_id = Pubkey::new(&process_id_vec);

    let params = json!([format!("{}", config_payer.id.pubkey())]);
    check_balance(39, &rpc_client, params); // config_payer balance
    let params = json!([format!("{}", process_id)]);
    check_balance(11, &rpc_client, params); // contract balance
    let params = json!([format!("{}", bob_pubkey)]);
    check_balance(0, &rpc_client, params); // recipient balance

    // Sign transaction by config_witness
    config_witness.command = WalletCommand::Witness(bob_pubkey, process_id);
    process_command(&config_witness).unwrap();

    let params = json!([format!("{}", config_payer.id.pubkey())]);
    check_balance(39, &rpc_client, params); // config_payer balance
    let params = json!([format!("{}", process_id)]);
    check_balance(1, &rpc_client, params); // contract balance
    let params = json!([format!("{}", bob_pubkey)]);
    check_balance(10, &rpc_client, params); // recipient balance

    server_exit();
    remove_dir_all(ledger_path).unwrap();
}

#[test]
fn test_wallet_cancel_tx() {
    let (server, leader_data, alice, ledger_path) = new_fullnode("test_wallet_cancel_tx");
    let server_exit = server.run(None);
    let bob_pubkey = Keypair::new().pubkey();

    let (sender, receiver) = channel();
    run_local_drone(alice, sender);
    let drone_addr = receiver.recv().unwrap();

    let rpc_client = RpcClient::new_from_socket(leader_data.rpc);

    let mut config_payer = WalletConfig::default();
    config_payer.drone_port = drone_addr.port();
    config_payer.rpc_port = leader_data.rpc.port();

    let mut config_witness = WalletConfig::default();
    config_witness.drone_port = drone_addr.port();
    config_witness.rpc_port = leader_data.rpc.port();

    assert_ne!(config_payer.id.pubkey(), config_witness.id.pubkey());

    request_and_confirm_airdrop(&rpc_client, &drone_addr, &config_payer.id, 50).unwrap();

    // Make transaction (from config_payer to bob_pubkey) requiring witness signature from config_witness
    config_payer.command = WalletCommand::Pay(
        10,
        bob_pubkey,
        None,
        None,
        Some(vec![config_witness.id.pubkey()]),
        Some(config_payer.id.pubkey()),
    );
    let sig_response = process_command(&config_payer);

    let object: Value = serde_json::from_str(&sig_response.unwrap()).unwrap();
    let process_id_str = object.get("processId").unwrap().as_str().unwrap();
    let process_id_vec = bs58::decode(process_id_str)
        .into_vec()
        .expect("base58-encoded public key");
    let process_id = Pubkey::new(&process_id_vec);

    let params = json!([format!("{}", config_payer.id.pubkey())]);
    check_balance(39, &rpc_client, params); // config_payer balance
    let params = json!([format!("{}", process_id)]);
    check_balance(11, &rpc_client, params); // contract balance
    let params = json!([format!("{}", bob_pubkey)]);
    check_balance(0, &rpc_client, params); // recipient balance

    // Sign transaction by config_witness
    config_payer.command = WalletCommand::Cancel(process_id);
    process_command(&config_payer).unwrap();

    let params = json!([format!("{}", config_payer.id.pubkey())]);
    check_balance(49, &rpc_client, params); // config_payer balance
    let params = json!([format!("{}", process_id)]);
    check_balance(1, &rpc_client, params); // contract balance
    let params = json!([format!("{}", bob_pubkey)]);
    check_balance(0, &rpc_client, params); // recipient balance

    server_exit();
    remove_dir_all(ledger_path).unwrap();
}
