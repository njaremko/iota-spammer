extern crate clap;
extern crate failure;
extern crate iota_lib_rs;
extern crate num_cpus;
extern crate term_size;
extern crate openssl_probe;

use std::sync::mpsc::sync_channel;
use std::thread;
use std::time::{Instant, Duration};

use clap::{App, Arg};
use failure::Error;
use iota_lib_rs::iota_api;
use iota_lib_rs::iri_api;
use iota_lib_rs::iri_api::responses;
use iota_lib_rs::model::*;
use iota_lib_rs::utils::trytes_converter;

fn main() -> Result<(), Error> {
    openssl_probe::init_ssl_cert_env_vars();
    let matches = App::new("Iota Spammer")
        .version("0.0.1")
        .author("Nathan J. <nathan@jaremko.ca>")
        .about("Spams the Iota Network")
        .arg(
            Arg::with_name("remote")
                .short("r")
                .long("remote")
                .help("Sets which IRI to spam (might need to be http/https...I haven't tested with UDP)")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("address")
                .short("a")
                .long("address")
                .help("Sets which address to spam")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("threads")
                .short("t")
                .long("threads")
                .help("Sets how many threads to use for PoW")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("queue")
                .short("q")
                .long("queue")
                .help("Number of transactions to approve requests to queue")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("weight")
                .short("w")
                .long("weight")
                .help("Sets the min weight threshold")
                .takes_value(true),
        )
        .get_matches();

    let trytes =
        "RUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTR";

    let uri = matches
        .value_of("remote")
        .unwrap_or("https://field.carriota.com");
    let address: String = matches.value_of("address").unwrap_or(trytes).into();
    let threads_str = matches.value_of("threads").unwrap_or_default();
    let queue_str = matches.value_of("queue").unwrap_or_default();
    let queue_size = if !queue_str.is_empty() {
        let mut tmp: usize = queue_str.parse()?;
        if tmp > 0 {
            tmp
        } else {
            5
        }
    } else {
        5
    };
    let weight_str = matches.value_of("weight").unwrap_or_default();
    let threads = if threads_str.is_empty() {
        num_cpus::get()
    } else {
        threads_str.parse()?
    };
    let weight = if !weight_str.is_empty() {
        let mut tmp: usize = weight_str.parse()?;
        if tmp < 9 {
            9
        } else if tmp > 14 {
            14
        } else {
            tmp
        }
    } else {
        14
    };

    let message = trytes_converter::to_trytes("Hello World").unwrap();
    let mut transfer = Transfer::default();
    transfer.set_value(0);
    transfer.set_address(address.clone());
    transfer.set_message(message);

    let mut terminal_width = 30;
    if let Some((w, _)) = term_size::dimensions() {
        terminal_width = w;
    } else {
        println!(
            "Couldn't determine terminal width...guessing {}",
            terminal_width
        );
    }
    let title_w = (terminal_width - 14) / 2;
    let title_style = "*".repeat(title_w);

    println!("{} Iota Spammer {}", title_style, title_style);
    println!("Spamming IRI: {}", uri);
    println!("PoW Threads: {}", threads);
    println!("Min Weight Magnitude: {}", weight);
    println!("Queue size: {}", queue_size);
    println!(
        "Spamming address: {}...",
        address
            .chars()
            .take(terminal_width - 22)
            .collect::<String>()
    );
    println!("{}", "*".repeat(terminal_width));

    // Create a bounded channel and feed it results till it's full (in the background)
    let (tx, rx) = sync_channel::<responses::GetTransactionsToApprove>(queue_size);
    let t_uri = uri.to_owned();
    thread::spawn(move || {
        loop {
            tx.send(iri_api::get_transactions_to_approve(&t_uri, 3, &None).unwrap()).unwrap();
            thread::sleep(Duration::from_millis(100));
        }
    });

    // Iterate over the transactions to approve and do PoW
    let mut i = 0;
    for tx_to_approve in rx.iter() {
        let api = iota_api::API::new(uri);
        
        let prepared_trytes =
            api.prepare_transfers(trytes, (&transfer).into(), None, &None, None, None)?;

        let before = Instant::now();
        let trytes_list = iri_api::attach_to_tangle_local(
            Some(threads),
            &tx_to_approve.trunk_transaction().unwrap(),
            &tx_to_approve.branch_transaction().unwrap(),
            weight,
            &prepared_trytes,
        )?.trytes()
            .unwrap();

        api.store_and_broadcast(&trytes_list)?;

        let tx: Vec<Transaction> = trytes_list
            .iter()
            .map(|trytes| trytes.parse().unwrap())
            .collect();

        let after = Instant::now();
        println!("Transaction {}: {:?}", i, tx[0].hash().unwrap());
        println!("Took {} seconds", after.duration_since(before).as_secs());
        i += 1;
    }
    Ok(())
}
