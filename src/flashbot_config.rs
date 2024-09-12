use std::collections::HashMap;
use reqwest::Client;
use serde_json::json;
use tokio::sync::{futures, mpsc};
use anyhow::Result;
use tokio::sync::mpsc::{Receiver, Sender};

const UNISWAP_V2_ROUTER: &str = "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D";
const UNISWAP_V3_ROUTER: &str = "0xE592427A0AEce92De3Edee1F18E0157C05861564";

pub struct AlchemyConfig {
    api: String,
    client: Client

}

impl AlchemyConfig {
    pub(crate) fn init(key: &str)-> Self {
        Self{
            api: key.to_string(),
            client: Client::new(),
        }
    }
    pub(crate) async fn get_pending_txs(&self) -> Result<Receiver<String>>{
        // step 1 get alchemy filter
        let create_filter_request = json!({
        "jsonrpc": "2.0",
        "method": "eth_newPendingTransactionFilter",
        "params": [],
        "id": 1
    });

        let filter_response = self.client
            .post(&self.api)
            .json(&create_filter_request)
            .send()
            .await?;

        let filter_id = filter_response.json::<serde_json::Value>().await?["result"]
            .as_str()
            .expect("Expected a filter ID")
            .to_string();

        println!("Filter ID: {}", filter_id);

      let config_copy = AlchemyConfig {
          api: self.api.clone(),
          client: self.client.clone(),
      };

    let (send, recv) = mpsc::channel(100);

         tokio::spawn(loop_query_pending_txs(config_copy, send, filter_id));


    Ok(recv)
    }


}


async fn loop_query_pending_txs(config: AlchemyConfig, send: Sender<String>, filter_id: String) {
    loop {
        // Step 2: get txs by filter
        let get_filter_changes_request = json!({
            "jsonrpc": "2.0",
            "method": "eth_getFilterChanges",
            "params": [filter_id],
            "id": 1
        });

        let pending_txs_response = config.client
            .post(&config.api)
            .json(&get_filter_changes_request)
            .send()
            .await.unwrap();

        let pending_txs = pending_txs_response.json::<serde_json::Value>().await.unwrap()["result"]
            .as_array()
            .unwrap_or(&vec![]) // 若没有新挂起交易时处理
            .iter()
            .map(|v| v.as_str().expect("Expected a string").to_string())
            .collect::<Vec<_>>();

       // println!("pending tx are {:?}", pending_txs);

        // init func map
        let mut function_map = HashMap::new();

        // uniswap v2 router contract func
        function_map.insert("0x38ed1739", "swapExactTokensForTokens");
        function_map.insert("0x8803dbee", "swapTokensForExactTokens");
        function_map.insert("0x7ff36ab5", "swapExactETHForTokens");
        function_map.insert("0x4a25d94a", "swapTokensForExactETH");
        function_map.insert("0x18cbafe5", "swapExactTokensForETH");
        function_map.insert("0xfb3bdb41", "swapETHForExactTokens");
        function_map.insert("0x414bf389", "exactInputSingle");
        function_map.insert("0xb858183f", "exactInput");
        function_map.insert("0xdb3e2198", "exactOutputSingle");
        function_map.insert("0x5023b4df", "exactOutput");
        function_map.insert("0x791ac947", "swapExactTokensForTokens");

        // filter tx hash
        for tx_hash in pending_txs {
            let client = config.client.clone();
            let alchemy_url =config.api.to_string();
            let function_map= function_map.clone();

            tokio::spawn(async move {
                let get_tx_request = json!({
                    "jsonrpc": "2.0",
                    "method": "eth_getTransactionByHash",
                    "params": [tx_hash.clone()],
                    "id": 1
                });

                let tx_response = match client
                    .post(&alchemy_url)
                    .json(&get_tx_request)
                    .send()
                    .await
                {
                    Ok(res) => res,
                    Err(err) => {
                        eprintln!("Failed to fetch transaction {}: {:?}", tx_hash, err);
                        return;
                    }
                };

                let tx_details = match tx_response.json::<serde_json::Value>().await {
                    Ok(details) => details,
                    Err(err) => {
                        eprintln!("Failed to parse transaction details {}: {:?}", tx_hash, err);
                        return;
                    }
                };

                // check tx is uniswap v2/v3
                if let Some(to_address) = tx_details["result"]["to"].as_str() {
                    if to_address.eq_ignore_ascii_case(UNISWAP_V2_ROUTER) || to_address.eq_ignore_ascii_case(UNISWAP_V3_ROUTER) {
                        println!("Uniswap Transaction details: {:?}", tx_details);

                        if let Some(input) = tx_details["result"]["input"].as_str() {
                            println!("input is [{}]", input);
                            let function_selector = &input[0..10]; // 跳过 "0x" 前缀
                            if let Some(function_name) = function_map.get(function_selector) {
                                println!("The transaction is calling the Uniswap function: {}", function_name);
                            } else {
                                println!("Unknown function selector: {}", function_selector);
                            }
                        }
                    }
                }
            });
        }

    }
}



#[cfg(test)]
mod tests {
    use ethabi::{decode, ParamType, Token};
    use hex::decode as hex_decode;

    #[test]
    fn decode_input() {
        // Hexadecimal string of the input data
        let data = "0x791ac9470000000000000000000000000000000000000000000000007fc0828298f04bc000000000000000000000000000000000000000000000000000d60cd34077b4a000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000474a6ff4cb4376a945995bd426826484380a7084000000000000000000000000000000000000000000000000000001918a1a385f0000000000000000000000000000000000000000000000000000000000000002000000000000000000000000f93fe087a1f38e9556c3e84776202b54bf633cb3000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";

        // Remove "0x" prefix and decode the hex string
        let data_bytes = hex_decode(&data[2..]).expect("Invalid hex string");

        // ABI of the `swapExactTokensForTokens` function
        let params = vec![
            ParamType::Uint(256),       // amountIn
            ParamType::Uint(256),       // amountOutMin
            ParamType::Array(Box::new(ParamType::Address)), // path
            ParamType::Address,         // to
            ParamType::Uint(256),       // deadline
        ];

        let decoded = decode(&params, &data_bytes[4..]).expect("Decoding failed");

        // Display the decoded values
        println!("Decoded values:");
        match &decoded[0] {
            Token::Uint(value) => println!("amountIn: {}", value),
            _ => panic!("Unexpected type for amountIn"),
        }
        match &decoded[1] {
            Token::Uint(value) => println!("amountOutMin: {}", value),
            _ => panic!("Unexpected type for amountOutMin"),
        }
        match &decoded[2] {
            Token::Array(values) => {
                println!("path:");
                for token in values {
                    if let Token::Address(addr) = token {
                        println!(" - 0x{}", hex::encode(addr));
                    }
                }
            }
            _ => panic!("Unexpected type for path"),
        }
        match &decoded[3] {
            Token::Address(value) => println!("to: 0x{}", hex::encode(value)),
            _ => panic!("Unexpected type for to"),
        }
        match &decoded[4] {
            Token::Uint(value) => println!("deadline: {}", value),
            _ => panic!("Unexpected type for deadline"),
        }
    }

}
