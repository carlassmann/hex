//! A German dictation often switches to English for whole sentences, and
//! the model writes their hesitations down ("Okay, um now I know that uh…").
//! "um" is an ordinary German word ("um zwölf", "um die URL"), so most
//! fillers leave only the sentences that read as English: more common
//! English words than German ones, and at least two. "uh" is no German word
//! and leaves every sentence.

const ENGLISH: &[&str] = &[
    "the", "and", "to", "a", "is", "it", "that", "we", "i", "you", "please", "can", "this", "of",
    "for", "should", "be", "do", "on", "with", "what", "are", "my", "so", "now", "just", "think",
];
const GERMAN: &[&str] = &[
    "der", "die", "das", "und", "ich", "ist", "nicht", "es", "wir", "zu", "den", "mit", "auf",
    "ein", "eine", "bitte", "dass", "auch", "sich", "du", "wenn", "noch", "dann", "aber", "mal",
    "jetzt", "einfach", "mir", "kannst", "haben", "sind",
];
const MIN_ENGLISH_WORDS: usize = 2;
const NEVER_GERMAN: &[&str] = &["uh", "uhh", "erm"];

/// `transcript` with fillers removed from its English sentences only.
pub fn strip_fillers(transcript: &str) -> String {
    sentences(transcript)
        .map(|(sentence, gap)| {
            let sentence = if reads_english(sentence) {
                super::filler_words::apply(sentence)
            } else {
                super::filler_words::apply_words(sentence, NEVER_GERMAN)
            };
            sentence + gap
        })
        .collect()
}

fn reads_english(sentence: &str) -> bool {
    let words = sentence
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase);
    let (english, german) = words.fold((0, 0), |(english, german), word| {
        (
            english + usize::from(ENGLISH.contains(&word.as_str())),
            german + usize::from(GERMAN.contains(&word.as_str())),
        )
    });
    english >= MIN_ENGLISH_WORDS && english > german
}

/// Each sentence with the whitespace after it.
fn sentences(text: &str) -> impl Iterator<Item = (&str, &str)> {
    let mut rest = text;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let end = sentence_end(rest);
        let gap_end = rest[end..]
            .find(|c: char| !c.is_whitespace())
            .map_or(rest.len(), |offset| end + offset);
        let piece = (&rest[..end], &rest[end..gap_end]);
        rest = &rest[gap_end..];
        Some(piece)
    })
}

fn sentence_end(text: &str) -> usize {
    let mut chars = text.char_indices().peekable();
    while let Some((index, c)) = chars.next() {
        if c == '\n' {
            return index;
        }
        if matches!(c, '.' | '!' | '?')
            && chars.peek().is_none_or(|&(_, next)| next.is_whitespace())
        {
            return index + c.len_utf8();
        }
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::strip_fillers;

    #[test]
    fn english_sentences_lose_their_fillers() {
        assert_eq!(
            strip_fillers("Um, so I think we should uh meet on Tuesday."),
            "So I think we should meet on Tuesday."
        );
        assert_eq!(
            strip_fillers("Okay, um now I know that uh it works."),
            "Okay, now I know that it works."
        );
    }

    #[test]
    fn german_sentences_keep_um() {
        for german in [
            "Die machen erst um zwölf auf.",
            "Mach einen Review-Step, um den PR vorzubereiten.",
            "Ah, jetzt sehe ich es.",
        ] {
            assert_eq!(strip_fillers(german), german);
        }
        assert_eq!(
            strip_fillers("Wenn wir Text in uh Slack schreiben."),
            "Wenn wir Text in Slack schreiben."
        );
    }

    #[test]
    fn a_mixed_dictation_is_judged_per_sentence() {
        assert_eq!(
            strip_fillers(
                "Please um open a new thread for this.\nDie machen erst um zwölf auf. Okay, uh can you do that?"
            ),
            "Please open a new thread for this.\nDie machen erst um zwölf auf. Okay, can you do that?"
        );
    }
}
