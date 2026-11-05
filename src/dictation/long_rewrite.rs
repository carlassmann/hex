//! A long dictation's rewrite in pieces, several at once. One rewrite of
//! minutes of speech outlasts any deadline and then falls back whole; pieces
//! of a few sentences each finish in seconds, and one that fails falls back
//! alone.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

/// About a minute of speech.
const PIECE_CHARS: usize = 1_000;
/// Rewrites sent to OpenCode at once.
const PARALLEL_REWRITES: usize = 6;

pub(crate) struct Rewritten {
    pub text: String,
    pub pieces: usize,
    pub fallbacks: Vec<String>,
}

/// Whether `text` is long enough to rewrite in pieces.
pub(crate) fn is_long(text: &str) -> bool {
    text.len() > PIECE_CHARS * 3 / 2
}

/// Rewrites each piece of `text` with `rewrite`, which returns the piece's
/// text or why it fell back; a fallen-back piece keeps its input.
pub(crate) fn rewrite(
    text: &str,
    rewrite: impl Fn(&str) -> Result<String, String> + Sync,
) -> Rewritten {
    let pieces = split(text);
    let results: Vec<Mutex<Option<Result<String, String>>>> =
        pieces.iter().map(|_| Mutex::new(None)).collect();
    let next = AtomicUsize::new(0);
    thread::scope(|scope| {
        for _ in 0..PARALLEL_REWRITES.min(pieces.len()) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(piece) = pieces.get(index) else {
                        break;
                    };
                    *results[index].lock().expect("rewrite result") = Some(rewrite(piece));
                }
            });
        }
    });

    let mut fallbacks = Vec::new();
    let texts: Vec<String> = pieces
        .iter()
        .zip(results)
        .map(
            |(piece, result)| match result.into_inner().expect("rewrite result") {
                Some(Ok(text)) => text,
                Some(Err(reason)) => {
                    fallbacks.push(reason);
                    (*piece).to_owned()
                }
                None => (*piece).to_owned(),
            },
        )
        .collect();
    Rewritten {
        text: join(&texts),
        pieces: pieces.len(),
        fallbacks,
    }
}

/// Cuts after a sentence once a piece reaches its size, else at a space, so
/// no word is split.
fn split(text: &str) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut rest = text.trim();
    while rest.len() > PIECE_CHARS * 3 / 2 {
        let cut = sentence_end_after(rest, PIECE_CHARS)
            .filter(|&cut| cut <= PIECE_CHARS * 2)
            .or_else(|| space_after(rest, PIECE_CHARS))
            .unwrap_or(rest.len());
        pieces.push(rest[..cut].trim());
        rest = rest[cut..].trim_start();
    }
    if !rest.is_empty() {
        pieces.push(rest);
    }
    pieces
}

fn sentence_end_after(text: &str, at: usize) -> Option<usize> {
    let mut previous = None;
    for (index, character) in text.char_indices() {
        if index > at
            && character.is_whitespace()
            && matches!(previous, Some('.' | '?' | '!' | '\n'))
        {
            return Some(index);
        }
        previous = Some(character);
    }
    None
}

fn space_after(text: &str, at: usize) -> Option<usize> {
    text.char_indices()
        .find(|&(index, character)| index > at && character.is_whitespace())
        .map(|(index, _)| index)
}

/// Pieces that hold lines, such as a numbered list, stay paragraphs apart.
pub(crate) fn join(texts: &[String]) -> String {
    let mut joined = String::new();
    for text in texts {
        if !joined.is_empty() {
            let separator = if joined.contains('\n') || text.contains('\n') {
                "\n\n"
            } else {
                " "
            };
            joined.push_str(separator);
        }
        joined.push_str(text.trim());
    }
    joined
}

#[cfg(test)]
mod tests {
    use super::*;

    fn speech(sentences: usize) -> String {
        (0..sentences)
            .map(|index| format!("This is sentence number {index} of a long dictation."))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn short_text_is_one_piece() {
        let text = speech(5);
        assert!(!is_long(&text));
        assert_eq!(split(&text), vec![text.as_str()]);
    }

    #[test]
    fn long_text_is_cut_after_sentences_and_keeps_every_word() {
        let text = speech(200);
        let pieces = split(&text);
        assert!(pieces.len() > 5);
        for piece in &pieces {
            assert!(piece.ends_with('.'), "{piece}");
            assert!(piece.len() <= PIECE_CHARS * 2);
        }
        assert_eq!(pieces.join(" "), text);
    }

    #[test]
    fn text_without_sentences_is_cut_at_spaces() {
        let text = "word ".repeat(1_000);
        let pieces = split(&text);
        assert!(pieces.len() > 1);
        assert_eq!(pieces.join(" "), text.trim());
    }

    #[test]
    fn a_failed_piece_keeps_its_text_and_the_others_are_rewritten() {
        let text = speech(200);
        let rewritten = rewrite(&text, |piece| {
            if piece.contains("number 0 ") {
                Err("timed out".into())
            } else {
                Ok(piece.to_uppercase())
            }
        });
        assert_eq!(rewritten.fallbacks, vec!["timed out".to_owned()]);
        assert!(rewritten.text.starts_with("This is sentence number 0"));
        assert!(rewritten.text.ends_with("NUMBER 199 OF A LONG DICTATION."));
        assert_eq!(rewritten.text.to_lowercase(), text.to_lowercase());
    }
}
