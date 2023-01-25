#![allow(clippy::too_many_arguments)]

use cfg_if::cfg_if;
use color_eyre::eyre;
use parity_scale_codec::Decode;
pub use subxt;
use subxt::{
    config::WithExtrinsicParams,
    events::StaticEvent,
    ext::sp_core,
    ext::sp_runtime::MultiAddress,
    tx::{BaseExtrinsicParams, PlainTip, TxInBlock, TxPayload, TxProgress, TxStatus},
    OnlineClient, SubstrateConfig,
};
use tap::Pipe;

use sp_core::sr25519;

cfg_if! {
    if #[cfg(feature = "old-substrate")] {
        #[subxt::subxt(runtime_metadata_path = "./old-creditcoin-metadata.scale")]
        pub mod creditcoin {}
    } else {
        #[subxt::subxt(runtime_metadata_path = "./creditcoin-metadata.scale")]
        pub mod creditcoin {}
    }
}

pub type ExtrinsicParams = BaseExtrinsicParams<SubstrateConfig, PlainTip>;

pub type CreditcoinConfig = WithExtrinsicParams<SubstrateConfig, ExtrinsicParams>;

pub type ApiClient = OnlineClient<CreditcoinConfig>;

pub type AccountId = subxt::ext::sp_core::crypto::AccountId32;

pub type Address = MultiAddress<AccountId, ()>;

pub type PairSigner<P> = subxt::tx::PairSigner<CreditcoinConfig, P>;

pub enum TxState {
    Included(TxInBlock<CreditcoinConfig, ApiClient>),
    Dropped,
}

pub async fn wait_for_in_block(
    mut progress: TxProgress<CreditcoinConfig, ApiClient>,
) -> color_eyre::Result<TxState> {
    let mut in_block = None;
    while let Some(status) = progress.next_item().await {
        let status = status?;

        match status {
            TxStatus::InBlock(ib) => {
                in_block = Some(ib);
                break;
            }
            TxStatus::Dropped => {
                return Ok(TxState::Dropped);
            }
            _ => {}
        }
    }
    let in_block = in_block.ok_or_else(|| eyre::eyre!("tx status subscription ended"))?;

    Ok(TxState::Included(in_block))
}

pub enum TxOutcome<E> {
    Success(Option<E>),
    Dropped,
}

#[derive(Decode)]
pub struct DontCare;

impl StaticEvent for DontCare {
    const PALLET: &'static str = "NONE";

    const EVENT: &'static str = "NONE";
}

pub async fn wait_for_success<E: StaticEvent>(
    progress: TxProgress<CreditcoinConfig, ApiClient>,
) -> eyre::Result<TxOutcome<E>> {
    let in_block = match wait_for_in_block(progress).await? {
        TxState::Included(in_block) => in_block,
        TxState::Dropped => return Ok(TxOutcome::Dropped),
    };
    let success = in_block.wait_for_success().await?;
    Ok(success.find_first::<E>()?.pipe(TxOutcome::Success))
}

pub async fn send_extrinsic<T: TxPayload>(
    tx: T,
    api: &ApiClient,
    signer: &PairSigner<sr25519::Pair>,
) -> color_eyre::Result<()> {
    let progress = api
        .tx()
        .sign_and_submit_then_watch_default(&tx, signer)
        .await?;

    wait_for_success::<DontCare>(progress).await?;

    Ok(())
}

#[cfg(not(feature = "old-substrate"))]
mod weight_impls {
    use crate::creditcoin;
    use subxt::ext::sp_runtime::traits::One;

    impl One for creditcoin::runtime_types::sp_weights::weight_v2::Weight {
        fn one() -> Self {
            Self {
                ref_time: 1,
                proof_size: 1,
            }
        }
    }
    impl std::ops::Mul for creditcoin::runtime_types::sp_weights::weight_v2::Weight {
        type Output = Self;

        fn mul(self, rhs: Self) -> Self::Output {
            Self {
                ref_time: self.ref_time * rhs.ref_time,
                proof_size: self.proof_size * rhs.proof_size,
            }
        }
    }
}
