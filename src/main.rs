extern crate iota_lib_rs;

use std::time::Instant;

use iota_lib_rs::iota_api;
use iota_lib_rs::model::*;
use iota_lib_rs::utils::trytes_converter;

fn main() {
    let trytes =
        "RUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTR";
    let message = trytes_converter::to_trytes("Hello World").unwrap();
    let mut transfer = Transfer::default();
    transfer.set_value(0);
    transfer.set_address(trytes);
    transfer.set_message(message);

    let mut i = 0;
    loop {
        let api = iota_api::API::new("https://field.carriota.com");
        let before = Instant::now();
        let tx =
            api.send_transfers(
                trytes, 3, 14, &transfer, true, None, &None, &None, None, None,
            ).unwrap();
        let after = Instant::now();
        println!("Transaction {}: {:?}", i, tx[0].hash());
        println!("Took {} seconds", after.duration_since(before).as_secs());
        i += 1;
    }
}
