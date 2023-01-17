green := "\\033[0;32m"
blue := "\\033[0;34m"
clear := "\\033[0m"

send-runtime-upgrade WASM_PATH: (run "send-extrinsic set-code " + WASM_PATH)

run ARGS:
    cargo run --release -- {{ARGS}}

install:
    @echo '{{blue}} Installing creditcoin-cli'
    cargo install --path ./crates/cli

# installs prerequisites for updating metadata
@install-subxt-cli:
    echo '{{blue}}Checking prerequisites...{{clear}}'
    command -v subxt >/dev/null 2>&1 || { echo '{{blue}}Installing subxt-cli...{{clear}}'; cargo install subxt-cli; }

# fetches latest chain metadata from a locally running node
update-metadata: install-subxt-cli
    @echo '{{blue}}Fetching latest metadata from local node...{{clear}}'
    subxt metadata -f bytes > ./crates/creditcoin-subxt/creditcoin-metadata.scale
    @touch ./crates/creditcoin-subxt/src/lib.rs
