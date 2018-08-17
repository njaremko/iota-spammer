#![feature(rust_2018_preview)]
#![feature(futures_api)]
#![feature(async_await)]
#![feature(await_macro)]
#![feature(mpsc_select)]
#![feature(nll)]
#![feature(duration_as_u128)]

extern crate clap;
extern crate failure;
extern crate iota_lib_rs;
extern crate num_cpus;
extern crate openssl_probe;
extern crate reqwest;
extern crate term_size;

use std::sync::atomic;
use std::thread;
use std::time::Instant;

use clap::{App, Arg};
use failure::Error;
use iota_lib_rs::iota_api;
use iota_lib_rs::iri_api;
use iota_lib_rs::iri_api::responses;
use iota_lib_rs::model::*;
use iota_lib_rs::utils::trytes_converter;

use futures::channel::mpsc;
use futures::executor::block_on;
use futures::prelude::*;
use futures::spawn;
use reqwest::Client;

fn main() -> Result<(), Error> {
    openssl_probe::init_ssl_cert_env_vars();
    let matches = App::new("Iota Spammer")
        .version("0.0.12")
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
    let start = Instant::now();

    let (mut approval_tx, approval_rx) = mpsc::channel(queue_size);
    let (pow_tx, pow_rx) = mpsc::channel(queue_size);
    let client = Client::new();
    let tx_uri = uri.to_string();

    // Start a thread to populate the tx_to_approve channel
    thread::spawn(move || loop {
        match block_on(iri_api::get_transactions_to_approve(
            &client,
            tx_uri.clone(),
            3,
            reference.clone(),
        )) {
            Ok(tx_to_approve) => block_on(approval_tx.send(tx_to_approve.clone())).unwrap(),
            Err(e) => eprintln!("gTTA Error: {}", e),
        };
    });

    // Start processing transactions
    block_on(processor(
        uri.to_string(),
        approval_rx,
        pow_tx,
        pow_rx,
        address,
        transfer,
        threads_to_use,
        weight,
    ));
    Ok(())
}

async fn processor(
    uri: String,
    approval_rx: mpsc::Receiver<responses::GetTransactionsToApprove>,
    pow_tx: mpsc::Sender<Vec<String>>,
    pow_rx: mpsc::Receiver<Vec<String>>,
    address: String,
    transfer: Transfer,
    threads_to_use: usize,
    weight: usize,
) {
    let tx_count = atomic::AtomicUsize::new(0);
    // Clone uri here because we need to move values into async functions (so they live long enough)
    let broadcast_uri = uri.clone();
    // Spawn the worker to handle proof of work, note that it is sequential because
    // trying to do many PoW at once is probably dumb
    spawn!(approval_rx.for_each(move |tx_to_approve| prepare_transfer(
        uri.clone(),
        tx_to_approve,
        pow_tx.clone(),
        address.clone(),
        transfer.clone(),
        threads_to_use,
        weight,
    ))).unwrap();

    await!(
        pow_rx.for_each_concurrent(num_cpus::get(), |pow_trytes| store_and_broadcast(
            broadcast_uri.clone(),
            pow_trytes,
            &tx_count
        ))
    );
}

async fn prepare_transfer(
    uri: String,
    tx_to_approve: responses::GetTransactionsToApprove,
    mut pow_tx: mpsc::Sender<Vec<String>>,
    address: String,
    transfer: Transfer,
    threads_to_use: usize,
    weight: usize,
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
                    pow_tx.try_send(powed_trytes.trytes().unwrap()).unwrap();
                }
                Err(e) => eprintln!("Prepare Transfers Error: {}", e),
            };
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}

async fn store_and_broadcast(uri: String, pow_trytes: Vec<String>, count: &atomic::AtomicUsize) {
    let api = iota_api::API::new(&uri);
    match await!(api.store_and_broadcast(pow_trytes.clone())) {
        Ok(()) => {
            let tx: Vec<Transaction> = pow_trytes
                .iter()
                .map(|trytes| trytes.parse().unwrap())
                .collect();
            count.fetch_add(1, atomic::Ordering::SeqCst);
            println!(
                "Transaction {}: {:?}",
                count.load(atomic::Ordering::SeqCst),
                tx[0].hash().unwrap()
            );
        }
        Err(e) => eprintln!("Broadcast Error: {}", e),
    }
}
