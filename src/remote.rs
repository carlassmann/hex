//! Hex on another machine: pairing devices with this one (`pairing`), this
//! Mac's connection to a paired Hex (`client`), and the settings it mirrors
//! and edits there (`settings`).

pub mod client;
pub mod pairing;
pub mod settings;

use std::sync::{Arc, OnceLock};

use crate::personal_commands::TransformationClient;

static TRANSFORMATIONS: OnceLock<Arc<TransformationClient>> = OnceLock::new();

/// Shares the app's transformation client, so a paired device's dictation
/// runs the same `hex.config.ts` transformations as one dictated here.
pub fn share_transformations(client: Arc<TransformationClient>) {
    let _ = TRANSFORMATIONS.set(client);
}

/// The app's transformation client; `None` in `hex service`, which runs no
/// personal-commands host.
pub(crate) fn transformations() -> Option<&'static TransformationClient> {
    TRANSFORMATIONS.get().map(Arc::as_ref)
}
