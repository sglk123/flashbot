mod flashbot_config;

use tokio::signal;


#[tokio::main]
async fn main() {
    let alchemy_url = "https://eth-mainnet.g.alchemy.com/v2/se3wRdEB5YDCAvi_Wkned0T34yENlTZw";
    let alchemy = flashbot_config::AlchemyConfig::init(alchemy_url);

    alchemy.get_pending_txs().await.unwrap();

    signal::ctrl_c().await.expect("Failed to listen for event");
}
