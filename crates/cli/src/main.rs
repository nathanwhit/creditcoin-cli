use async_trait::async_trait;
use cfg_if::cfg_if;
use clap::Parser;
use color_eyre::{eyre::eyre, Report};

use creditcoin_subxt::{
    creditcoin::{self},
    subxt::{self},
    AccountId, Address, ApiClient, PairSigner,
};
use std::{fmt, path::PathBuf, str::FromStr};
use subxt::ext::sp_core;
use subxt::ext::sp_runtime::traits::One;

use sp_core::{crypto::Ss58Codec, sr25519, Pair};

// imports that vary based on the version of substrate
cfg_if! {
    if #[cfg(feature = "old-substrate")] {
        use creditcoin::runtime_types::creditcoin_node_runtime::Call as RuntimeCall;
        type Weight = u64;
    } else {
        use creditcoin::runtime_types::creditcoin_node_runtime::RuntimeCall;
        use creditcoin::runtime_types::sp_weights::weight_v2::Weight;
    }
}

#[derive(Clone)]
struct CreditcoinAccountId(AccountId);

impl fmt::Display for CreditcoinAccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_ss58check())
    }
}

impl FromStr for CreditcoinAccountId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let account =
            AccountId::from_ss58check(s).map_err(|_| format!("{s} is not a valid SS58 Address"))?;
        Ok(Self(account))
    }
}

impl From<CreditcoinAccountId> for Address {
    fn from(value: CreditcoinAccountId) -> Self {
        Self::from(value.0)
    }
}

impl From<CreditcoinAccountId> for AccountId {
    fn from(value: CreditcoinAccountId) -> Self {
        value.0
    }
}

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    cmd: Command,

    #[clap(long, default_value = "//Alice")]
    suri: String,
}

#[derive(clap::Subcommand)]
enum Command {
    #[clap(subcommand)]
    SendExtrinsic(Extrinsic),

    GetHead {
        #[clap(long, short)]
        quiet: bool,
    },

    GetVersion,

    CountStorageItems {
        module: String,
        name: String,
    },
}

#[derive(Clone)]
pub struct HexBytes(Vec<u8>);

impl FromStr for HexBytes {
    type Err = Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = if let Some(rest) = s.strip_prefix("0x") {
            rest
        } else {
            s
        };
        let bytes = hex::decode(value)?;
        Ok(Self(bytes))
    }
}

impl fmt::Debug for HexBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("HexBytes")
            .field(&hex::encode(&self.0))
            .finish()
    }
}

impl fmt::Display for HexBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", hex::encode(&self.0))
    }
}

#[async_trait]
impl Execute for Command {
    async fn execute(
        self,
        api: &ApiClient,
        signer: &PairSigner<sr25519::Pair>,
        sudo: &PairSigner<sr25519::Pair>,
    ) -> color_eyre::Result<()> {
        match self {
            Command::SendExtrinsic(ext) => ext.execute(api, signer, sudo).await,
            Command::GetHead { quiet } => {
                let hash = api
                    .rpc()
                    .block_hash(None)
                    .await?
                    .ok_or_else(|| eyre!("bad"))?;

                if quiet {
                    println!("{hash:?}");
                } else {
                    println!("Chain head: {hash:?}");
                }
                Ok(())
            }
            Command::GetVersion => {
                let version = api.rpc().runtime_version(None).await?;
                println!("{version:?}");
                Ok(())
            }
            Command::CountStorageItems { module, name } => {
                let address = creditcoin_subxt::subxt::storage::dynamic_root(module, name);

                let mut iter = api.storage().iter(address, 512, None).await?;
                let mut count = 0;
                while iter.next().await?.is_some() {
                    count += 1;
                }
                println!("{count}");
                Ok(())
            }
        }
    }
}

#[derive(clap::Subcommand)]
enum Extrinsic {
    AddAuthority {
        who: CreditcoinAccountId,
    },
    Transfer {
        to: CreditcoinAccountId,
        amount: f64,
    },
    SetBalance {
        account: CreditcoinAccountId,
        amount: f64,
    },
    SetCode {
        wasm_path: PathBuf,
    },
}

fn eyreify<E: fmt::Debug>(e: E) -> color_eyre::Report {
    color_eyre::eyre::eyre!("{e:?}")
}

const CREDO_PER_CTC: u128 = 1_000_000_000_000_000_000;

struct Ctc(u128);

fn scale(value: u128, by: f64) -> u128 {
    assert!(by >= 0.0);
    if by < 1.0 {
        let divisor = by.recip().round() as u128;
        value / divisor
    } else {
        let int = by.trunc() as u128;
        let frac = by.fract();

        (value * int) + scale(value, frac)
    }
}

fn ctc_frac(amount: f64) -> Ctc {
    Ctc(scale(CREDO_PER_CTC, amount))
}

#[async_trait]
trait Execute {
    async fn execute(
        self,
        api: &ApiClient,
        signer: &PairSigner<sr25519::Pair>,
        sudo: &PairSigner<sr25519::Pair>,
    ) -> color_eyre::Result<()>;
}

#[async_trait]
impl Execute for Extrinsic {
    async fn execute(
        self,
        api: &ApiClient,
        signer: &PairSigner<sr25519::Pair>,
        sudo: &PairSigner<sr25519::Pair>,
    ) -> color_eyre::Result<()> {
        use creditcoin::runtime_types as types;
        match self {
            Extrinsic::AddAuthority { who } => {
                use types::pallet_creditcoin::pallet::Call as CreditcoinCall;

                let tx = creditcoin::tx().sudo().sudo(RuntimeCall::Creditcoin(
                    CreditcoinCall::add_authority { who: who.into() },
                ));

                creditcoin_subxt::send_extrinsic(tx, api, sudo).await
            }
            Extrinsic::Transfer { to, amount } => {
                let Ctc(amount) = ctc_frac(amount);

                let tx = creditcoin::tx().balances().transfer(to.into(), amount);

                creditcoin_subxt::send_extrinsic(tx, api, signer).await
            }
            Extrinsic::SetBalance { account, amount } => {
                use types::pallet_balances::pallet::Call as BalancesCall;

                let Ctc(amount) = ctc_frac(amount);

                let tx = creditcoin::tx().sudo().sudo(RuntimeCall::Balances(
                    BalancesCall::set_balance {
                        who: account.into(),
                        new_free: amount,
                        new_reserved: 0,
                    },
                ));

                creditcoin_subxt::send_extrinsic(tx, api, sudo).await
            }
            Extrinsic::SetCode { wasm_path } => {
                use types::frame_system::pallet::Call as SystemCall;

                let code = tokio::fs::read(&wasm_path).await?;

                let weight = Weight::one();

                let tx = creditcoin::tx().sudo().sudo_unchecked_weight(
                    RuntimeCall::System(SystemCall::set_code { code }),
                    weight,
                );

                println!("Waiting for transaction to be included in a block...");
                creditcoin_subxt::send_extrinsic(tx, api, sudo).await
            }
        }
    }
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    let signer = sr25519::Pair::from_string(&cli.suri, None).map_err(eyreify)?;

    let sudo = PairSigner::new(sr25519::Pair::from_string("//Alice", None).map_err(eyreify)?);

    let signer = PairSigner::new(signer);

    let api = ApiClient::new().await?;

    cli.cmd.execute(&api, &signer, &sudo).await?;

    Ok(())
}
