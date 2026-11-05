//! A paired device's stream rewritten while it still records. Each time its
//! transcribed windows add up to a piece (about ten seconds of speech ending
//! in a sentence at a pause), the piece goes through the Modes
//! with a deadline long enough for OpenCode's slow answers, which hide
//! behind the speaker still talking. At the finish only the text since the
//! last piece is rewritten while the device waits; a piece that never
//! answered keeps the words as heard, so nothing said is lost.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::transcription::stream::{Segment, Stream};

/// About ten seconds of speech: a piece ends at the first pause after this
/// that also ends a sentence, so each rewrite sees whole thoughts and the
/// finish has only the last few seconds left to rewrite.
const PIECE_CHARS: usize = 150;
/// A piece ends at the next pause past this, sentence or not.
const MAX_PIECE_CHARS: usize = 600;
/// A piece rewritten while the device records may take this long.
const RECORDING_DEADLINE: Duration = Duration::from_secs(60);
const WATCH: Duration = Duration::from_secs(1);

struct Piece {
    raw: String,
    rewritten: Option<super::journal::Rewrite>,
}

#[derive(Default)]
struct State {
    pieces: Vec<Piece>,
    /// Segments already in a piece.
    taken: usize,
    finished: bool,
}

pub(super) struct EarlyRewrite {
    state: Mutex<State>,
    changed: Condvar,
}

fn running() -> MutexGuard<'static, HashMap<u64, Arc<EarlyRewrite>>> {
    static RUNNING: OnceLock<Mutex<HashMap<u64, Arc<EarlyRewrite>>>> = OnceLock::new();
    RUNNING
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

/// Starts rewriting `stream` as it is transcribed, once per stream.
pub(super) fn watch(stream: Arc<Stream>, application: String) {
    let early = {
        let mut running = running();
        if running.contains_key(&stream.id) {
            return;
        }
        let early = Arc::new(EarlyRewrite {
            state: Mutex::default(),
            changed: Condvar::new(),
        });
        running.insert(stream.id, early.clone());
        early
    };
    let spawned = thread::Builder::new()
        .name("remote-early-rewrite".into())
        .spawn(move || {
            early.run(&stream, &application);
            if stream.ended() {
                running().remove(&stream.id);
            }
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "remote stream could not rewrite while recording");
    }
}

/// The rewrite started for stream `id`, which the finish takes over.
pub(super) fn take(id: u64) -> Option<Arc<EarlyRewrite>> {
    running().remove(&id)
}

impl EarlyRewrite {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn run(self: &Arc<Self>, stream: &Stream, application: &str) {
        loop {
            thread::sleep(WATCH);
            if self.lock().finished || stream.ended() {
                return;
            }
            let segments = stream.segments();
            let mut state = self.lock();
            let pending = &segments[state.taken.min(segments.len())..];
            let Some(count) = piece_length(pending) else {
                continue;
            };
            let raw = joined(&pending[..count]);
            state.taken += count;
            let index = state.pieces.len();
            state.pieces.push(Piece {
                raw: raw.clone(),
                rewritten: None,
            });
            drop(state);
            self.rewrite_piece(index, raw, application.to_owned());
        }
    }

    fn rewrite_piece(self: &Arc<Self>, index: usize, raw: String, application: String) {
        let early = self.clone();
        let spawned = thread::Builder::new()
            .name("remote-piece-rewrite".into())
            .spawn(move || {
                let started = Instant::now();
                let processed = super::process(&raw, application, Some(RECORDING_DEADLINE));
                let latency_ms = started.elapsed().as_millis() as u64;
                tracing::info!(
                    piece = index,
                    chars = raw.len(),
                    latency_ms,
                    "remote stream piece rewritten while recording"
                );
                let rewritten = super::journal::Rewrite::new("piece", &raw, &processed, latency_ms);
                early.lock().pieces[index].rewritten = Some(rewritten);
                early.changed.notify_all();
            });
        if let Err(error) = spawned {
            tracing::warn!(%error, piece = index, "remote stream piece kept as heard");
        }
    }

    /// The text of the whole stream, given its final `segments`: the pieces
    /// rewritten while it recorded and `rewrite_rest` of the segments after
    /// them. Waits at most `wait` for pieces still with OpenCode, then keeps
    /// those as heard.
    pub(super) fn finish(
        &self,
        segments: &[Segment],
        wait: Duration,
        rewrite_rest: impl FnOnce(&str) -> crate::dictation::processor::Processed,
    ) -> Finished {
        let rest = {
            let mut state = self.lock();
            state.finished = true;
            joined(&segments[state.taken.min(segments.len())..])
        };
        let deadline = Instant::now() + wait;
        let rest = (!rest.is_empty()).then(|| {
            let started = Instant::now();
            let processed = rewrite_rest(&rest);
            let latency_ms = started.elapsed().as_millis() as u64;
            super::journal::Rewrite::new("rest", &rest, &processed, latency_ms)
        });
        let mut state = self.lock();
        while state.pieces.iter().any(|piece| piece.rewritten.is_none()) {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            state = self
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(|error| error.into_inner())
                .0;
        }
        let kept_as_heard = state
            .pieces
            .iter()
            .filter(|piece| piece.rewritten.is_none())
            .count();
        let mut rewrites: Vec<super::journal::Rewrite> = state
            .pieces
            .iter()
            .map(|piece| {
                piece.rewritten.clone().unwrap_or(super::journal::Rewrite {
                    stage: "piece",
                    input: piece.raw.clone(),
                    output: None,
                    observation: None,
                    transformations: Vec::new(),
                    latency_ms: 0,
                })
            })
            .collect();
        rewrites.extend(rest);
        let texts: Vec<String> = rewrites
            .iter()
            .map(|rewrite| {
                rewrite
                    .output
                    .clone()
                    .unwrap_or_else(|| rewrite.input.clone())
            })
            .filter(|text| !text.trim().is_empty())
            .collect();
        Finished {
            text: crate::dictation::long_rewrite::join(&texts),
            pieces: state.pieces.len(),
            kept_as_heard,
            rewrites,
        }
    }
}

pub(super) struct Finished {
    pub text: String,
    /// Pieces rewritten while the stream recorded.
    pub pieces: usize,
    pub kept_as_heard: usize,
    /// Every piece's trip through the Modes, then the rest's.
    pub rewrites: Vec<super::journal::Rewrite>,
}

/// How many of the `pending` segments, each cut at a pause, make the next
/// piece, if they make one yet.
fn piece_length(pending: &[Segment]) -> Option<usize> {
    let mut chars = 0;
    for (index, segment) in pending.iter().enumerate() {
        chars += segment.text.len() + 1;
        let ends_sentence = segment.text.trim_end().ends_with(['.', '?', '!', '…']);
        if (chars >= PIECE_CHARS && ends_sentence) || chars >= MAX_PIECE_CHARS {
            return Some(index + 1);
        }
    }
    None
}

fn joined(segments: &[Segment]) -> String {
    segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segments(texts: &[&str]) -> Vec<Segment> {
        texts
            .iter()
            .enumerate()
            .map(|(segment, text)| Segment {
                segment,
                start_ms: 0,
                end_ms: 0,
                text: (*text).to_owned(),
            })
            .collect()
    }

    #[test]
    fn a_piece_waits_for_enough_speech() {
        assert_eq!(piece_length(&segments(&["Short one."])), None);
    }

    #[test]
    fn a_piece_ends_at_the_first_sentence_end_past_its_length() {
        let long = "word ".repeat(40);
        let pending = segments(&[&long, "and then it goes on", "until here.", "Next."]);
        assert_eq!(piece_length(&pending), Some(3));
    }

    #[test]
    fn a_piece_without_a_sentence_end_ends_at_its_cap() {
        let long = "word ".repeat(70);
        let pending = segments(&[&long, &long, "more"]);
        assert_eq!(piece_length(&pending), Some(2));
    }
}
