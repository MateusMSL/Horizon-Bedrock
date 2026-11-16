//! What debug builds check of the far terrain's bookkeeping.

use super::Lod;
use std::sync::Arc;

impl Lod {
    /// `mesh_samples` holds every active tile's ready samples and nothing else (debug builds).
    pub(super) fn check_mesh_samples(&self) {
        let kept = self.mesh_samples.iter().count();
        let mut want = 0;
        for key in &self.active {
            if let Some(columns) = self.ready.get(key) {
                want += 1;
                let got = self.mesh_samples.get(key);
                assert!(
                    got.is_some_and(|got| Arc::ptr_eq(got, columns)),
                    "mesh samples of {key:?} out of date"
                );
            }
        }
        assert_eq!(
            kept, want,
            "mesh samples keep tiles no longer active or ready"
        );
    }
}
