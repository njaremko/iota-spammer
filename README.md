# iota-spammer

This is an iota spammer.

It spams https://field.carriota.com/ by default

PoW is done locally

```
Iota Spammer 0.0.1
Nathan J. <nathan@jaremko.ca>
Spams the Iota Network

USAGE:
    iota-spammer [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -a, --address <address>    Sets which address to spam
    -q, --queue <queue>        Number of transactions to approve requests to queue
    -r, --remote <remote>      Sets which IRI to spam (might need to be http/https...I haven't tested with UDP)
    -t, --threads <threads>    Sets how many threads to use for PoW
    -w, --weight <weight>      Sets the min weight threshold
```