<p align="center">
<h1 align="center">Tyto</h1>
</p>

<div align="center">
    <a href="https://travis-ci.com/adcrn/tyto"><img
    src="https://travis-ci.com/adcrn/tyto.svg?token=9jG6XKKRPepsyqdsCqW7&branch=master"
    alt="Travis-CI"></a>
</div>
<br>

_Tyto_ is an open source BitTorrent tracker written in [Rust](https://www.rust-lang.org). It aims to be safe, performant, and rock-solid.

## Why?
Tyto was created to facilitate the distribution of media through the
BitTorrent protocol. Many of the popular tracker software available has not
been updated in more than a decade. Many aspects that were "nice-to-have" at
the time, e.g. IPv6 support, multithreading, etc., have become quite necessary at the scale at which modern deployments operate. Tyto implements
many of the newest official standards and de facto extensions in order to
create a robust and performant distibution system that can be used to 
serve many swarms with minimal downtime.

## Features
- [ ] Configuration hot-reloading
- [x] Global metrics
- [x] IPv4 and IPv6 support
- [ ] Private tracker support
- [ ] Storage-agnostic backend
- [ ] Swarm statistics

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

## Performance
The tracker makes heavy use of `async/await` and does its best to reduce excessive allocation of objects. The following stats were achieved on a 2017 MacBook Pro:

```
❯ wrk -t3 -c90 -d1m -s wrk_load_test.lua http://localhost:6666
Running 1m test @ http://localhost:6666
  3 threads and 90 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency     3.89ms  815.50us  22.07ms   97.30%
    Req/Sec     7.79k   462.48     8.76k    81.17%
  1395683 requests in 1.00m, 512.99MB read
Requests/sec:  23258.34
Transfer/sec:      8.55MB
```

The stress-testing procedure can be found in `wrk_load_test.lua` and makes use of the [wrk](https://github.com/wg/wrk) program.

## License
MIT

## Similar Projects
- [Chihaya](https://github.com/chihaya/chihaya) - a tracker written in Go
- [Ocelot](https://github.com/WhatCD/Ocelot) - a tracker written in C++
