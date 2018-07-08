extern crate clap;
extern crate failure;
extern crate iota_lib_rs;
extern crate num_cpus;
extern crate term_size;

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
    let matches = App::new("Iota Spammer")
        .version("0.0.1")
        .author("Nathan J. <nathan@jaremko.ca>")
        .about("Spams the Iota Network")
        .arg(
            Arg::with_name("remote")
                .short("r")
                .long("remote")
                .help("Sets which IRI to spam (must be http/https)")
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
        .get_matches();

    let trytes =
        "RUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTRUSTR";

    let uri = matches
        .value_of("remote")
        .unwrap_or("https://field.carriota.com");
    let address: String = matches.value_of("address").unwrap_or(trytes).into();
    let threads_str = matches.value_of("threads").unwrap_or_default();

    let threads = if threads_str.is_empty() {
        num_cpus::get()
    } else {
        threads_str.parse()?
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
    println!(
        "Spamming address: {}...",
        address
            .chars()
            .take(terminal_width - 22)
            .collect::<String>()
    );
    println!("{}", "*".repeat(terminal_width));

    let (tx, rx) = sync_channel::<responses::GetTransactionsToApprove>(5);
    let t_uri = uri.to_owned();
    thread::spawn(move || {
        loop {
            tx.send(iri_api::get_transactions_to_approve(&t_uri, 3, &None).unwrap()).unwrap();
            thread::sleep(Duration::from_millis(500));
        }
    });

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
            14,
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
