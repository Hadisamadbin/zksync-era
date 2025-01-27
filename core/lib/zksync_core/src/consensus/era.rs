//! This module provides convenience functions to run consensus components in different modes
//! as expected by the zkSync Era.
//!
//! This module simply glues APIs that are already publicly exposed by the `consensus` module,
//! so in case any custom behavior is needed, these APIs should be used directly.

use std::sync::Arc;

use zksync_concurrency::{ctx, limiter, time};
use zksync_dal::{ConnectionPool, Core};

use super::{
    config::{Config, Secrets},
    fetcher::Fetcher,
    storage::Store,
};
use crate::sync_layer::{sync_action::ActionQueueSender, MainNodeClient, SyncState};

/// Runs the consensus task in the main node mode.
pub async fn run_main_node(
    ctx: &ctx::Ctx,
    cfg: super::MainNodeConfig,
    pool: ConnectionPool<Core>,
) -> anyhow::Result<()> {
    // Consensus is a new component.
    // For now in case of error we just log it and allow the server
    // to continue running.
    if let Err(err) = cfg.run(ctx, Store(pool)).await {
        tracing::error!(%err, "Consensus actor failed");
    } else {
        tracing::info!("Consensus actor stopped");
    }
    Ok(())
}

/// Runs the consensus in the fetcher mode (e.g. for the external node needs).
/// The fetcher implementation may either be p2p or centralized.
pub async fn run_fetcher(
    ctx: &ctx::Ctx,
    cfg: Option<(Config, Secrets)>,
    pool: ConnectionPool<Core>,
    sync_state: SyncState,
    main_node_client: Arc<dyn MainNodeClient>,
    actions: ActionQueueSender,
) -> anyhow::Result<()> {
    let fetcher = Fetcher {
        store: Store(pool),
        sync_state: sync_state.clone(),
        client: main_node_client,
        limiter: limiter::Limiter::new(
            ctx,
            limiter::Rate {
                burst: 10,
                refresh: time::Duration::milliseconds(30),
            },
        ),
    };
    let res = match cfg {
        Some((cfg, secrets)) => fetcher.run_p2p(ctx, actions, cfg.p2p(&secrets)?).await,
        None => fetcher.run_centralized(ctx, actions).await,
    };
    tracing::info!("Consensus actor stopped");
    res
}
