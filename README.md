# BitMask Core
Core functionality for the BitMask wallet - <https://bitmask.app>

**BitMask** is a bitcoin wallet and a browser extension for accessing decentralized web applications on the Bitcoin blokchain. It is designed to support UTXO based smart contracting protocols such as RGB, with planned support for Omni layer, TARO and many others.


[![Crates.io](https://img.shields.io/crates/v/bitmask-core?style=flat-square)](https://docs.rs/bitmask-core/latest/bitmask-core/)
[![npm: bitmask-core](https://img.shields.io/npm/v/bitmask-core?style=flat-square)](https://www.npmjs.com/package/bitmask-core)
[![License: MIT+APACHE](https://img.shields.io/crates/l/bitmask-core?style=flat-square)](https://mit-license.org)
[![Telegram](https://img.shields.io/badge/telegram-invite-blue?style=flat-square)](https://t.me/+eQk5aQ5--iUxYzVk)

## Uses

- [bdk](https://github.com/bitcoindevkit/bdk) - Bitcoin Dev Kit
- [gloo](https://github.com/rustwasm/gloo)
- [wasm-pack](https://github.com/rustwasm/wasm-pack)

## Build

This should work with either wasm-pack, trunk, or x86-64.

Some environment variables may be needed in order to compile on macos-aarch64, for more, [see this](https://github.com/sapio-lang/sapio/issues/146#issuecomment-960659800).

If there are issues compiling, be sure to check you're compiling with the latest Rust version.

To build this as a NodeJS module, use: `wasm-pack build --release --target bundler`

## Test

1. Lint against wasm32: `cargo clippy --target wasm32-unknown-unknown`
2. Run tests in browser: `TEST_WALLET_SEED="replace with a 12 word mnemonic for a wallet containing testnet sats" wasm-pack test --headless --chrome`

## Run

To run the bitmaskd node with REST server, either for testing the web wallet, or simply for increased privacy:

`cargo install --features=server --path .`

Then run `bitmaskd`.
