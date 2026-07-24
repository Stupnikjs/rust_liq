use std::sync::atomic::{AtomicU64, Ordering};

const BUCKET_SECS: u64 = 60;          // 1 bucket par minute
const NUM_BUCKETS: usize = 60;         // 60 buckets = 1h de fenêtre max

pub struct Bucket {
    epoch: AtomicU64,      // numéro de bucket (unix_time / BUCKET_SECS) actuellement stocké ici
    attempts: AtomicU64,
    successes: AtomicU64,
}

impl Bucket {
    pub fn new() -> Self {
        Self {
            epoch: AtomicU64::new(0),
            attempts: AtomicU64::new(0),
            successes: AtomicU64::new(0),
        }
    }

    /// Reset le bucket si on est passé dans une nouvelle tranche de temps,
    /// puis incrémente. Le check-then-reset n'est pas parfaitement atomique
    /// entre plusieurs threads concurrents sur le même bucket au même instant,
    /// mais au pire tu perds un incrément isolé pendant la transition — négligeable
    /// pour un taux affiché en monitoring.
    pub fn record(&self, current_epoch: u64, success: bool) {
        let stored_epoch = self.epoch.load(Ordering::Relaxed);
        if stored_epoch != current_epoch {
            // nouvelle tranche de temps : on écrase l'ancienne
            self.attempts.store(0, Ordering::Relaxed);
            self.successes.store(0, Ordering::Relaxed);
            self.epoch.store(current_epoch, Ordering::Relaxed);
        }
        self.attempts.fetch_add(1, Ordering::Relaxed);
        if success {
            self.successes.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn read(&self, current_epoch: u64) -> (u64, u64) {
        let stored_epoch = self.epoch.load(Ordering::Relaxed);
        if stored_epoch != current_epoch
            && stored_epoch + 1 != current_epoch // bucket "juste précédent" reste valide pour lecture
        {
            // Le bucket ne correspond ni à l'epoch courant ni au précédent
            // → il est trop vieux, considère-le comme vide plutôt que
            // d'afficher des vieilles données obsolètes.
            return (0, 0);
        }
        (
            self.attempts.load(Ordering::Relaxed),
            self.successes.load(Ordering::Relaxed),
        )
    }
}