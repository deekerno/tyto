<p align="center">
    <img alt="Tyto" title="Tyto" src="https://i.imgur.com/D9Lq6K2.png"
    width="150">
<h1 align="center">Tyto</h1>
</p>

<div align="center">
    <a href="https://travis-ci.com/adcrn/tyto"><img
    src="https://travis-ci.com/adcrn/tyto.svg?token=9jG6XKKRPepsyqdsCqW7&branch=master"
    alt="Travis-CI"></a>
</div>
<br>

__Disclaimer:__ This software must only be used in accordance with the laws of your respective country.

_Tyto_ is an open source BitTorrent tracker written in [Rust](https://www.rust-lang.org). It aims to be safe, performant, and rock-solid.

## Why?
Tyto was created to facilitate the distribution of (legal) media through the
BitTorrent protocol. Many of the popular tracker software available has not
been updated in more than a decade. Many aspects that were "nice-to-have" at
the time, e.g. IPv6 support, multithreading, etc., have become quite necessary at the scale at which modern deployments operate. Tyto implements
many of the newest official standards and de facto extensions in order to
create a robust and performant distibution system that can be used to legally
serve many swarms with minimal downtime.

## (Planned) Features
- [x] Asynchronous operation
- [x] Multithreading
- [x] IPv4 and IPv6 support
- [ ] Storage-agnostic backend
- [ ] Swarm statistics
- [ ] Private tracker support

## Usage
### Building
Tyto requires the [latest stable version of Rust](https://www.rust-lang.org/learn/get-started).

```sh
$ git clone git@github.com:adcrn/tyto.git
$ cd tyto
```

Edit `config.toml` to the desired configuration, and then build the program. Testing is also available.

```sh
$ cargo test
$ cargo build --release
```

### Running
Make sure that the storage backend and path have been correctly added to the configuration before starting the program. Then start it up! The `-c` flag is also available to provide an alternate path to a configuration file.

```sh
$ ./target/release/tyto
```

By default, Tyto will output information about incoming requests for logging purposes. This can be piped to a file for later inspection. Running the program prefaced by `RUST_LOG=error` will reduce the output to just critical errors.

## Performance
No rigorous benchmarking has been done on Tyto, but some cursory performance testing has been done. This was done on a 2.3 GHz Intel Core i5 Macbook, and essentially mimicking a situation in which all requests, which each represent a unique peer, are for one torrent and create a lot of churn for the swarm by causing resource contention.

```
$ wrk -t3 -c90 -d1m -s wrk_load_test.lua http://localhost:6666
Running 1m test @ http://localhost:6666
  3 threads and 90 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency     3.10ms    4.25ms 117.21ms   89.58%
    Req/Sec    14.36k     1.71k   30.90k    84.33%
  2573078 requests in 1.00m, 529.40MB read
Requests/sec:  42835.36
Transfer/sec:      8.81MB
```

## Notes
### Statistics
In order to aid with metrics and things like swarm health, each swarm has a
statistics structure associated with it that can run analyze the swarm for
anamolies and calculate certain measures. All methodologies are extensively
commented, and can easily be extended.

### Storage Backend
Tyto has been developed to be _storage-agnostic_. It does not require one to
lock themselves to a certain storage solution, and users are free to implement
their own solutions. In order to facilitate ease of implementation, a
convenience trait named _PeerStorage_ is required in order to add a solution
with minimal stress. Tyto does come with an in-memory peer store already
implemented, and there is a _TorrentStorage_ trait if needed.

## License
MIT

## Similar Projects
- [Chihaya](https://github.com/chihaya/chihaya) - a tracker written in Go
- [Ocelot](https://github.com/WhatCD/Ocelot) - a tracker written in C++
