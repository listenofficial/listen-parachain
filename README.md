<p align="center">
  <img src="docs/listen-logo.jpeg?raw=true" alt="image" width="230"/>
</p>

<h3 align="center">Decentralized social platform</h3>

<div align="center">


[![Substrate version](https://img.shields.io/badge/Substrate-3.0.0-brightgreen?logo=Parity%20Substrate)](https://substrate.dev/)
[![GitHub license](https://img.shields.io/badge/license-MIT%2FApache2-blue)](LICENSE)

</div>
<!-- TOC -->

- [1. Introduce](#1-introduce)
- [2. Getting Started](#2-getting-started)
- [3. Learn More](#3-learn-more)

<!-- /TOC -->

## 1. Introduce
Listen is a decentralized social platform.

***
## 2. Getting Started
Follow these steps to get started with the Parachain Node.
***
* Rust Setup
`curl https://sh.rustup.rs -sSf | sh`
***
* Clone Project From Github
```buildoutcfg
git clone https://github.com/listenofficial/listen-parachain.git
```
* Build
```buildoutcfg
cd listen-parachain

cargo build --release
```


* Run (Connect to the main network)
```buildoutcfg
cd target/release
```
```buildoutcfg
./listen-collator --collator --base-path db --pruning archive --state-cache-size=0 -- --execution wasm --chain kusama --unsafe-pruning --pruning=1000 --state-cache-size=0
```
## 3. Learn More
* [Listen Official Website](https://listen.io)
* Provids the RPC for Url [https://polkadot.js.org/apps](https://polkadot.js.org/apps/)
```buildoutcfg
wss://rpc.mainnet.listen.io
wss://wss.mainnet.listen.io
```
* [Listen Mainnet Explorer](https://scan.listen.io)
* [Twitter](https://twitter.com/Listen_io)
* [Telegram](https://t.me/listengroup)








