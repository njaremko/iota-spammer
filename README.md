# iota-spammer

This is an iota spammer.

It spams https://trinity.iota.fm by default

PoW is done locally

```
Iota Spammer 0.0.11
Nathan J. <nathan@jaremko.ca>
Spams the Iota Network

USAGE:
    iota-spammer [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -a, --address <address>        Sets which address to spam
    -i, --iri <iri>                Sets which IRI to spam (might need to be http/https...I haven't tested with UDP)
    -m, --message <message>        Sets message for spam transactions
    -q, --queue <queue>            Number of transactions to approve requests to queue
    -r, --reference <reference>    Sets the reference TX
    -t, --threads <threads>        Sets how many threads to use for PoW
    -w, --weight <weight>          Sets the min weight threshold
```