#[cfg(feature = "gpu")]
pub mod availability_checker {
    use prover_dal::{ConnectionPool, Prover, ProverDal};
    use zksync_types::prover_dal::{GpuProverInstanceStatus, SocketAddress};

    use crate::metrics::{KillingReason, METRICS};

    pub struct AvailabilityChecker {
        address: SocketAddress,
        zone: String,
        polling_interval_secs: u32,
        pool: ConnectionPool<Prover>,
    }

    impl AvailabilityChecker {
        pub fn new(
            address: SocketAddress,
            zone: String,
            polling_interval_secs: u32,
            pool: ConnectionPool<Prover>,
        ) -> Self {
            Self {
                address,
                zone,
                polling_interval_secs,
                pool,
            }
        }

        pub async fn run(
            self,
            stop_receiver: tokio::sync::watch::Receiver<bool>,
        ) -> anyhow::Result<()> {
            while !*stop_receiver.borrow() {
                let status = self
                    .pool
                    .connection()
                    .await
                    .unwrap()
                    .fri_gpu_prover_queue_dal()
                    .get_prover_instance_status(self.address.clone(), self.zone.clone())
                    .await;

                match status {
                    None => {
                        METRICS.zombie_prover_instances_count[&KillingReason::Absent].inc();
                        tracing::info!(
                            "Prover instance at address {:?}, availability zone {} was not found in the database, shutting down",
                            self.address,
                            self.zone
                        );
                        // After returning from the task, it will shut down all the other tasks
                        return Ok(());
                    }
                    Some(GpuProverInstanceStatus::Dead) => {
                        METRICS.zombie_prover_instances_count[&KillingReason::Dead].inc();
                        tracing::info!(
                            "Prover instance at address {:?}, availability zone {} was found marked as dead, shutting down",
                            self.address,
                            self.zone
                        );
                        // After returning from the task, it will shut down all the other tasks
                        return Ok(());
                    }
                    Some(_) => (),
                }

                tokio::time::sleep(std::time::Duration::from_secs(
                    self.polling_interval_secs as u64,
                ))
                .await;
            }

            tracing::info!("Availability checker was shut down");

            Ok(())
        }
    }
}
