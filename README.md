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

_Tyto_ is an open source BitTorrent tracker written in [Rust](https://www.rust-lang.org). It aims to be safe, performant, and distributed.

## Why?
Tyto was created to facilitate the distribution of (legal) media through the
BitTorrent protocol. Many of the popular tracker software available has not
been updated in more than a decade. Many aspects that were "nice-to-have" at
the time, e.g. IPv6 support, multithreading, etc., have become quite necessary at the scale at which modern deployments operate. Tyto implements
many of the newest official standards and de facto extensions in order to
create a robust and performant distibution system that can be used to legally
serve many swarms with minimal downtime.

## Current Progress
The BitTorrent-specific parts of the codebase are finished and ready for testing. These modules do include IPv6 support as described in the BitTorrent specification. There are in-memory storage solutions already implemented, and need to undergo actual testing. There is a search underway for statistics crates that are easily used to analyze the data that will be produced by the storage backends.

## (Planned) Features
- Asynchronous operation
- Multithreading
- IPv4 and IPv6 support
- Storage-agnostic backend
- Distributed fault-tolerance
- Swarm statistics
- Private tracker support

## Notes
### Fault-Tolerance
A long term goal of the project is to enable high availability through
distributed fault-tolerance. There are many unforseen events that can take down a tracker
system, and spatial distribution of nodes running Tyto in a distributed fashion
would allow for continued uptime as fixes are applied to unreachable nodes.

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

### Testing
This project makes heavy use of testing. All test results for the lastest
commits can be checked through the build status of the project. To run tests
locally, clone the repo and run `cargo test`. 

## License
MIT

## Similar Projects
- [Chihaya](https://github.com/chihaya/chihaya) - a tracker written in Go
- [Ocelot](https://github.com/WhatCD/Ocelot) - a tracker written in C++
