<p align="center">
  <img src="docs/listen-logo.jpeg?raw=true" alt="image"/>
</p>

<h3 align="center">Decentralized social platform</h3>

<div align="center">


[![Substrate version](https://img.shields.io/badge/Substrate-3.0.0-brightgreen?logo=Parity%20Substrate)](https://substrate.dev/)
[![GitHub license](https://img.shields.io/badge/license-MIT%2FApache2-blue)](LICENSE)

</div>

## Introduce
Listen is a decentralized social platform.
***
## Getting Started
Follow these steps to get started with the Parachain Node.
***
### Rust Setup
`curl https://sh.rustup.rs -sSf | sh`
***
### Clone Project From Github
```buildoutcfg
git clone https://github.com/listenofficial/listen-parachain.git
```
### Build
```buildoutcfg
cd listen-parachain

cargo build --release
```

### Run
#### relay chain
```
./target/release/polkadot build-spec --chain rococo-local --disable-default-bootnode --raw > rococo-local-cfde.json
./polkadot  --chain rococo-local-cfde.json --alice --base-path alice-db --ws-port 9988 --port 40338 --node-key bd2f9ba01b71cae0cb641005b8f5a2c8ca0bcb41ce670300ad34829305244bec  --ws-external --rpc-external --rpc-methods=Unsafe --rpc-cors=all
./polkadot  --chain rococo-local-cfde.json --bob --base-path bob-db --ws-port 9989 --port 40339  --ws-external --rpc-external --rpc-methods=Unsafe –rpc-cors=all
```
> /ip4/47.108.199.133/tcp/30334/p2p/12D3KooWK7kGugDnzY92ZVqsRspk5LJQRyvGujnCaCwhBafeJi2k
#### parachain
```buildoutcfg
./listen-collator export-genesis-wasm > genesis-wasm
./listen-collator export-genesis-state > genesis-state
```
## PS
Some of our basic modules, such as multi-asset, cross-chain transfer, come from the [Acala Team](https://github.com/AcalaNetwork/Acala). And some have been modified. Thanks to their talented engineers for their outstanding contribution to the Polkadot community.


./subkey inspect ///xxx
./target/release/polkadot key insert --base-path /tmp/node01 \
--chain customSpecRaw.json \
--scheme Sr25519 \
--suri ///xxx \
--password-interactive \
--key-type aura
```




