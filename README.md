<p align="center">
  <img src="image/listen-logo.jpeg?raw=true" alt="image" width="230"/>
</p>

<h3 align="center">Decentralized social platform</h3>

<div align="center">

[![Substrate version](https://img.shields.io/badge/Substrate-3.0.0-brightgreen?logo=Parity%20Substrate)](https://substrate.dev/)
[![GitHub license](https://img.shields.io/badge/license-MIT%2FApache2-blue)](LICENSE)

</div>
<!-- TOC -->

- [1. Introduce](#1-introduce)
- [2. Getting Started](#2-getting-started)
  - [For common users](#option-1-for-common-users)
  - [For developers](#option-2-for-developers)

- [3. Learn More](#3-learn-more)

<!-- /TOC -->

## 1. Introduce
Listen is a decentralized social platform.

***
## 2. Getting Started
Follow these steps to get started with the Parachain Node.

***
### ***option 1 (For common users)***
* Download the executable file
```buildoutcfg
wget -O- https://github.com/listenofficial/listen-parachain/releases/latest  | grep -o '/.*listen-collator'  | sed  's/^/https:\/\/github.com/g' | xargs  wget -c
```
* Run (Connect to the main network)
```buildoutcfg
./listen-collator --collator --base-path db --pruning archive --state-cache-size=0 -- --execution wasm --chain kusama --unsafe-pruning --pruning=1000 --state-cache-size=0
```
***
### ***option 2 (For developers)***
> ***If you are a common user, you don't need this option***
* Rust
```buildoutcfg
curl https://sh.rustup.rs -sSf | sh
```

* Clone Project From Github
```buildoutcfg
git clone https://github.com/listenofficial/listen-parachain.git
```
* Init
```angular2html
cd listen-parachain
make submodule
```
* Build
```buildoutcfg
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
* [***Listen Official Website***](https://listen.io)
* Provids the RPC in https://polkadot.js.org/apps
	* [***wss://rpc.mainnet.listen.io***](https://polkadot.js.org/apps/?rpc=wss%3A%2F%2Fwss.mainnet.listen.io#/explorer)
	* [***wss://rpc.mainnet.listen.io***](https://polkadot.js.org/apps/?rpc=wss%3A%2F%2Frpc.mainnet.listen.io#/explorer)
* [***Listen Mainnet Explorer***](https://scan.listen.io)
* [***Twitter***](https://twitter.com/Listen_io)
* [***Telegram***](https://t.me/listengroup)








