# Blockchain for Bikes - Backend
We are building an application for trading bicycles, by utilizing the blockchain technology. We also plan to build a client application where the users can do transactions such as buying or selling a bicycle. We want to use a combination of technologies that all teammates have experience working on while keeping an interesting project that is technologically challenging.

In this system, there are two components, ![frontend](https://github.com/SEP-G5/Mobile-Client) and backend.

## Backend
In the backend we find the blockchain technology, along with an API for the frontend to query information about it. It also has a peer-to-peer (p2p) network, to provide a decentralized system. The blockchain technology has three parts to it. First we have transactions. They are were we put information about a bike transaction. These transactions are gathered up and put into blocks which is the next part. Blocks store information about a collection of transactions. To prevent malicious actions of trying to modify the history, these blocks are linked together, such that changing a block in the past, would also change all future blocks. This is what forms our last part, the blockchain. The blockchain is simply a collection of blocks, linked together.


# Build Instructions
This program is written in rust. You can ![install it here](https://www.rust-lang.org/tools/install).

## Required Packages
To build the project you might first need some packages.

On Ubuntu you need:
``` shell
apt install openssl libssl-dev pkg-config
```

## Building
To run the program.
``` shell
cargo run
```
Run the tests.
``` shell
cargo test -- --nocapture
```
To build with only compiler frontend.
``` shell
cargo check
```

# Roadmap
Here is an early estimate of our roadmap. They are high level blocks of work, which we will break down into tasks and work on them in week-long sprints.
![img](https://github.com/SEP-G5/Backend/blob/master/res/roadmap.png)
