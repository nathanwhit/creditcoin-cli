# creditcoin-cli

This is a CLI tool for performing miscellaneous interactions with the creditcoin blockchain.

This is mostly for personal/development usage when I want to, for instance, send a test transaction quickly from the command line.

## Usage

This assumes you have a working, up-to-date Rust toolchain.

If you have [just](https://github.com/casey/just) installed, you can install the CLI with

```bash
just install
```

or with plain cargo, from the root of the repo:

```bash
cargo install --path crates/cli
```

Either way, the binary will be installed using cargo, and can then be run with

```bash
creditcoin-cli -h
```

## Useful commands

### Sending a runtime upgrade to a local node

Helpful for testing runtime upgrades/migrations.

```bash
# against a development chain (or a chain that uses Alice for the sudo account)
creditcoin-cli send-extrinsic set-code <WASM_BLOB_PATH>
```

Where the `WASM_BLOB_PATH` is the path to the runtime WASM blob to upgrade to.

If you're upgrading a chain that _doesn't_ have Alice as the sudo account, you'll need to specify the SURI for the sudo account

```bash
creditcoin-cli --suri <SUDO_SURI> send-extrinsic set-code <WASM_BLOB_PATH>
```
