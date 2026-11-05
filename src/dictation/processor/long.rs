//! A long transcript's OpenCode rewrite in pieces (`dictation::long_rewrite`),
//! observed as one processing stage.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use super::{ProcessingObservation, Profile, Source, generate_observed, prompt};

use crate::dictation::context::ContextSnapshot;
use crate::dictation::vocabulary::VocabularyEntry;

/// A long transcript's rewrite, piece by piece (`long_rewrite`).
pub(super) fn rewrite_long(
    profile: &Profile,
    transcript: &str,
    context: &ContextSnapshot,
    vocabulary: &[VocabularyEntry],
    source: Source,
    cancelled: &AtomicBool,
) -> (Option<String>, ProcessingObservation) {
    let started = Instant::now();
    let rewritten = crate::dictation::long_rewrite::rewrite(transcript, |piece| {
        let prompt = prompt(profile, piece, context, vocabulary, source);
        let (text, observation) = generate_observed(
            &prompt,
            profile.model.as_ref(),
            profile.deadline,
            cancelled,
            &profile.name,
            Instant::now(),
        );
        text.ok_or_else(|| observation.fallback.unwrap_or_default())
    });
    tracing::info!(
        profile = profile.name,
        pieces = rewritten.pieces,
        fallbacks = rewritten.fallbacks.len(),
        latency_ms = started.elapsed().as_millis() as u64,
        "long dictation rewritten in pieces"
    );
    let fallback = (!rewritten.fallbacks.is_empty()).then(|| {
        format!(
            "{} of {} pieces kept as heard: {}",
            rewritten.fallbacks.len(),
            rewritten.pieces,
            rewritten.fallbacks[0]
        )
    });
    let all_fell_back = rewritten.fallbacks.len() == rewritten.pieces;
    let observation = ProcessingObservation {
        profile: profile.name.clone(),
        latency_ms: started.elapsed().as_millis() as u64,
        model: profile
            .model
            .as_ref()
            .map(|model| format!("{}/{}", model.provider, model.id)),
        fallback,
    };
    ((!all_fell_back).then_some(rewritten.text), observation)
}
