use std::collections::HashSet;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordDifficulty {
    Common,
    Normal,
    Complex,
}

#[rustfmt::skip]
const COMMON_WORDS_LIST: &[&str] = &[
    "the", "be", "to", "of", "and", "a", "in", "that", "have", "it",
    "for", "not", "on", "with", "he", "as", "you", "do", "at", "this",
    "but", "his", "by", "from", "they", "we", "say", "her", "she", "or",
    "an", "will", "my", "one", "all", "would", "there", "their", "what", "so",
    "up", "out", "if", "about", "who", "get", "which", "go", "me", "when",
    "make", "can", "like", "time", "no", "just", "him", "know", "take", "people",
    "into", "year", "your", "good", "some", "could", "them", "see", "other", "than",
    "then", "now", "look", "only", "come", "its", "over", "think", "also", "back",
    "after", "use", "two", "how", "our", "work", "first", "well", "way", "even",
    "new", "want", "because", "any", "these", "give", "day", "most", "us", "great",
    "been", "has", "had", "was", "were", "are", "did", "find", "here", "thing",
    "many", "much", "long", "too", "same", "right", "old", "big", "little", "world",
    "school", "still", "try", "last", "ask", "need", "hand", "high", "keep", "place",
    "start", "far", "turn", "away", "off", "left", "while", "might", "again", "does",
    "man", "call", "house", "point", "being", "end", "help", "show", "why", "home",
    "kind", "read", "life", "run", "move", "live", "play", "open", "let", "put",
    "set", "late", "three", "four", "five", "small", "large", "part", "made", "men",
    "few", "next", "found", "name", "word", "close", "city", "side", "along", "under",
    "went", "got", "near", "real", "money", "both", "head", "white", "line", "state",
    "night", "sure", "began", "once", "change", "water", "face", "door", "since", "used",
    "best", "until", "more", "where", "going", "really", "done", "enough", "yet", "another",
    "upon", "saw", "around", "always", "together", "often", "those", "own", "very", "down",
    "such", "each", "every", "before", "must", "through", "between", "should", "may", "tell",
    "never", "stop", "mean", "hard", "hold", "full", "seem", "light", "hear", "second",
    "room", "body", "young", "group", "without", "leave", "table", "number", "story", "above",
    "below", "care", "class", "early", "feel", "girl", "boy", "love", "food", "book",
    "form", "able", "idea", "free", "less", "case", "eye", "game", "air", "ago",
    "half", "power", "plan", "week", "today", "true",
];

fn common_words() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| COMMON_WORDS_LIST.iter().copied().collect())
}

const PUNCTUATION_CHARS: &str = ".,!?;:'\"-()[]{}/";

fn strip_punctuation(word: &str) -> &str {
    word.trim_matches(|c| PUNCTUATION_CHARS.contains(c))
}

pub fn get_word_difficulty(word: &str) -> WordDifficulty {
    let lower = word.to_lowercase();
    let trimmed = strip_punctuation(&lower);
    if common_words().contains(trimmed) {
        return WordDifficulty::Common;
    }
    let char_count = trimmed.chars().count();
    let has_complex = trimmed.chars().any(|c| matches!(c, 'z' | 'x' | 'q' | 'j'));
    if char_count > 8 || has_complex {
        WordDifficulty::Complex
    } else {
        WordDifficulty::Normal
    }
}
