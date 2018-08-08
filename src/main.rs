#![feature(rust_2018_preview)]
#![feature(rust_2018_idioms)]
#![feature(futures_api)]
#![feature(async_await)]
#![feature(await_macro)]

extern crate clap;
extern crate failure;
extern crate iota_lib_rs;
extern crate num_cpus;
extern crate openssl_probe;
extern crate reqwest;
extern crate term_size;

use std::sync::mpsc::sync_channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::SyncSender;
use std::thread;
use std::time::Instant;

use clap::{App, Arg};
use failure::Error;
use iota_lib_rs::iota_api;
use iota_lib_rs::iri_api;
use iota_lib_rs::iri_api::responses;
use iota_lib_rs::model::*;
use iota_lib_rs::utils::trytes_converter;

use futures::executor::block_on;
use futures::executor::spawn;
use futures::future::lazy;
use futures::prelude::*;

use crossbeam::channel as channel;

use reqwest::Client;

fn main() -> Result<(), Error> {
    openssl_probe::init_ssl_cert_env_vars();
    let matches = App::new("Iota Spammer")
        .version("0.0.8")
        .author("Nathan J. <nathan@jaremko.ca>")
        .about("Spams the Iota Network")
        .arg(
            Arg::with_name("reference")
                .short("r")
                .long("reference")
                .help("Sets the reference TX")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("iri")
                .short("i")
                .long("iri")
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
        .arg(
            Arg::with_name("message")
                .short("m")
                .long("message")
                .help("Sets message for spam transactions")
                .takes_value(true),
        )
        .get_matches();

    let trytes =
        "RUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTR";

    let uri = matches.value_of("iri").unwrap_or("https://trinity.iota.fm");
    let reference: Option<String> = match matches.value_of("reference") {
        Some(t) => Some(t.to_string()),
        None => None,
    };
    let address: String = matches.value_of("address").unwrap_or(trytes).into();
    let message = matches.value_of("message").unwrap_or("Hello World");
    let encoded_message = trytes_converter::to_trytes(message).unwrap();
    let threads_str = matches.value_of("threads").unwrap_or_default();
    let actual_thread_count = num_cpus::get();
    let threads_to_use = if !threads_str.is_empty() {
        let tmp: usize = threads_str.parse()?;
        if tmp > 0 && tmp <= actual_thread_count {
            tmp
        } else {
            actual_thread_count
        }
    } else {
        actual_thread_count
    };

    let queue_str = matches.value_of("queue").unwrap_or_default();
    let queue_size = if !queue_str.is_empty() {
        let tmp: usize = queue_str.parse()?;
        if tmp > 0 {
            tmp
        } else {
            5
        }
    } else {
        5
    };

    let weight_str = matches.value_of("weight").unwrap_or_default();
    let weight = if !weight_str.is_empty() {
        let tmp: usize = weight_str.parse()?;
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

    let mut transfer = Transfer::default();
    transfer.set_value(0);
    transfer.set_address(address.clone());
    transfer.set_message(encoded_message.clone());

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
    println!("PoW Threads: {}", threads_to_use);
    println!("Min Weight Magnitude: {}", weight);
    println!("Queue size: {}", queue_size);
    println!("Spam Message: {}", message);
    println!(
        "Spamming address: {}...",
        address
            .chars()
            .take(terminal_width - 22)
            .collect::<String>()
    );
    if let Some(reference) = &reference {
        println!("Reference TX: {}", reference);
    }
    println!("{}", "*".repeat(terminal_width));

    // Create a bounded channel and feed it results till it's full (in the background)
    let (approval_tx, approval_rx) = channel::bounded(queue_size);
    let t_uri = uri.to_owned();
    get_tx_to_approve_thread(t_uri, approval_tx, reference);

    let (pow_tx, pow_rx) = channel::bounded(queue_size);//sync_channel::<Vec<String>>(queue_size);
    let t_uri = uri.to_owned();
    prepare_transfers_thread(
        t_uri,
        pow_tx,
        approval_rx,
        address,
        transfer,
        threads_to_use,
        weight,
    );

    let (broadcast_tx, broadcast_rx) = channel::bounded(queue_size);
    let t_uri = uri.to_owned();
    store_and_broadcast_thread(t_uri, broadcast_tx, pow_rx);

    // Iterate over the transactions to approve and do PoW
    let mut before = Instant::now();
    for (i, sent_trytes) in broadcast_rx.enumerate() {
        let tx: Vec<Transaction> = sent_trytes
            .iter()
            .map(|trytes| trytes.parse().unwrap())
            .collect();

        println!("Transaction {}: {:?}", i, tx[0].hash().unwrap());
        if i > 0 && i % 10 == 0 {
            println!(
                "Average TXs/Sec: {:.2}",
                1_f64 / (Instant::now().duration_since(before).as_secs() as f64 / 10_f64)
            );
            before = Instant::now();
        }
    }
    Ok(())
}

fn get_tx_to_approve_thread(
    uri: String,
    approval_tx: channel::internal::channel::Sender<responses::GetTransactionsToApprove>,
    reference: Option<String>,
) {
    thread::spawn(move || {
        let client = Client::new();
        loop {
            match block_on(iri_api::get_transactions_to_approve(
                &client,
                uri.clone(),
                3,
                reference.clone(),
            )) {
                Ok(tx_to_approve) => {
                    approval_tx.send(tx_to_approve);
                }
                Err(e) => eprintln!("gTTA Error: {}", e),
            };
        }
    });
}

fn prepare_transfers_thread(
    uri: String,
    pow_tx: channel::internal::channel::Sender<Vec<String>>,
    approval_rx: channel::internal::channel::Receiver<responses::GetTransactionsToApprove>,
    address: String,
    transfer: Transfer,
    threads_to_use: usize,
    weight: usize,
) {
    thread::spawn(move || {
        for tx_to_approve in approval_rx {
            block_on(prepare_transfers_helper(
                uri.clone(),
                pow_tx.clone(),
                address.clone(),
                transfer.clone(),
                threads_to_use,
                weight,
                tx_to_approve,
            ));
        }
    });
}

async fn prepare_transfers_helper(
    uri: String,
    pow_tx: channel::internal::channel::Sender<Vec<String>>,
    address: String,
    transfer: Transfer,
    threads_to_use: usize,
    weight: usize,
    tx_to_approve: responses::GetTransactionsToApprove,
) {
    let api = iota_api::API::new(&uri);
    match await!(api.prepare_transfers(
        address.clone(),
        vec![transfer.clone()],
        None,
        None,
        None,
        None,
    )) {
        Ok(prepared_trytes) => {
            match await!(iri_api::attach_to_tangle_local(
                Some(threads_to_use),
                tx_to_approve.trunk_transaction().unwrap(),
                tx_to_approve.branch_transaction().unwrap(),
                weight,
                prepared_trytes,
            )) {
                Ok(powed_trytes) => {
                    pow_tx.send(powed_trytes.trytes().unwrap());
                }
                Err(e) => eprintln!("Prepare Transfers Error: {}", e),
            };
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}

fn store_and_broadcast_thread(
    uri: String,
    broadcast_tx: channel::internal::channel::Sender<Vec<String>>,
    pow_rx: channel::internal::channel::Receiver<Vec<String>>,
) {
    thread::spawn(move || {
        for pow_trytes in pow_rx {
            block_on(spawn(store_and_broadcast_helper(
                uri.clone(),
                broadcast_tx.clone(),
                pow_trytes,
            )));
        }
    });
}

async fn store_and_broadcast_helper(
    uri: String,
    broadcast_tx: channel::internal::channel::Sender<Vec<String>>,
    pow_trytes: Vec<String>,
) {
    let local_api = iota_api::API::new(&uri);
    if let Err(e) = await!(local_api.store_and_broadcast(pow_trytes.clone())) {
        eprintln!("Broadcast Error: {}", e);
    }
    broadcast_tx.send(pow_trytes);
}
