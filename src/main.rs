use std::env;

use clap::{Parser, Subcommand};
use dotenv::dotenv;
use drift_sdk::{
    dlob::dlob_builder::DLOBBuilder,
    memcmp::get_user_with_order_filter,
    slot_subscriber::SlotSubscriber,
    types::{MarketType, RpcSendTransactionConfig},
    usermap::UserMap,
    utils::{get_ws_url, load_keypair_multi_format},
    DriftClient, RpcAccountProvider,
};
use flashlight::jit_maker::{JitMaker, JitMakerConfig};
use rust::{jitter::Jitter, types::ComputeBudgetParams};
use solana_sdk::commitment_config::CommitmentConfig;
use tokio::sync::Mutex;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize user
    InitUser {},
    RunJit {},
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    dotenv().ok();
    env_logger::init();

    let endpoint = env::var("RPC_URL").expect("RPC_URL must be set");
    let private_key = env::var("PRIVATE_KEY").expect("SECRET_KEY must be set");
    let wallet =
        drift_sdk::Wallet::new(load_keypair_multi_format(&private_key).expect("valid keypair"));

    match cli.command {
        Commands::InitUser {} => {
            // let keypair = read_keypair_file(&private_key).expect("read keypair");
            // let secret_key_slices = keypair.secret().to_bytes();
            // let key = bs58::encode(keypair.secret()).into_string();
            // let key = String::from_utf8(secret_key_slices.to_vec()).unwrap();

            println!("{:?}", wallet.signer());
        }
        Commands::RunJit {} => {
            let sub_accounts = vec![0];

            let mut drift_client = DriftClient::new(
                drift_sdk::types::Context::DevNet,
                RpcAccountProvider::with_commitment(&endpoint, CommitmentConfig::finalized()),
                wallet,
            )
            .await
            .expect("drift client");

            for sub_account_id in sub_accounts.iter() {
                drift_client
                    .add_user(*sub_account_id)
                    .await
                    .expect("add user");
            }

            let rpc_config = RpcSendTransactionConfig::default();
            let cu_params = ComputeBudgetParams::new(100_000, 1_400_000);
            let jitter =
                Jitter::new_with_shotgun(drift_client.clone(), Some(rpc_config), Some(cu_params));

            let usermap = UserMap::new(
                CommitmentConfig::processed(),
                endpoint.clone(),
                true,
                Some(vec![get_user_with_order_filter()]),
            );

            let slot_subscriber = SlotSubscriber::new(get_ws_url(&endpoint).expect("valid url"));

            let dlob_builder =
                std::sync::Arc::new(Mutex::new(DLOBBuilder::new(slot_subscriber, usermap, 1)));

            let jit_maker_config = JitMakerConfig {
                // SOL
                market_indexes: vec![0],
                sub_account_ids: sub_accounts,
                target_leverage: 1.0,
                spread: -0.01,
                market_type: MarketType::Perp,
                drift_client,
                jitter,
                dlob_builder,
                volatility_threshold: 0.015,
            };

            let mut jit_maker = JitMaker::new(jit_maker_config).expect("jit maker");

            let _ = jit_maker.subscribe().await;

            println!("Hello, world!");
        }
    }
}
