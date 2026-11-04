//! A capture's file name: the date it was taken, then what the user calls it, as in
//! "2026-11-03 - Invoice". Until it's called something, the time it was taken
//! follows the date instead, as in "2026-11-03 14.32.05".

use std::ops::Range;

/// The most characters a name takes.
pub const MAX: usize = 100;

/// How long the date that starts the time a capture was taken is.
const DATE: usize = "2026-11-03".len();

/// What every name of the capture taken at `taken` starts with: its date and a
/// dash, as in "2026-11-03 - ".
pub fn prefix(taken: &str) -> String {
    let date = taken.get(..DATE).unwrap_or(taken);
    format!("{date} - ")
}

/// What typing `c` puts in a name: itself, a dash for the characters that separate
/// a date's or a path's parts, or nothing for the others a file name can't hold.
pub fn typed(c: char) -> Option<char> {
    match c {
        '/' | '\\' | ':' | '|' => Some('-'),
        '<' | '>' | '"' | '?' | '*' => None,
        c if c.is_control() => None,
        c => Some(c),
    }
}

/// The file name, without its extension, of the capture taken at `taken` and called
/// `name`: `taken` itself while it's called nothing. Windows drops the spaces and
/// dots that end a file name, so they go.
pub fn stem(taken: &str, name: &str) -> String {
    let name = name.trim_start().trim_end_matches([' ', '.']);
    if name.is_empty() {
        taken.to_owned()
    } else {
        format!("{}{name}", prefix(taken))
    }
}

/// What the capture taken at `taken`, whose file is called `stem`, is called: nothing
/// until the user names it.
pub fn name<'a>(taken: &str, stem: &'a str) -> &'a str {
    stem.strip_prefix(&prefix(taken)).unwrap_or_default()
}

/// A name as it's typed: its characters, the caret between two of them, and the
/// selection's other end, where the caret was when selecting started.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Field {
    chars: Vec<char>,
    caret: usize,
    anchor: usize,
}

impl Field {
    /// `text`, all selected, so typing replaces it.
    pub fn new(text: &str) -> Self {
        let chars: Vec<char> = text.chars().collect();
        Self {
            caret: chars.len(),
            anchor: 0,
            chars,
        }
    }

    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn len(&self) -> usize {
        self.chars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    /// The characters selected, which may be none.
    pub fn selection(&self) -> Range<usize> {
        self.caret.min(self.anchor)..self.caret.max(self.anchor)
    }

    pub fn selected(&self) -> String {
        self.chars[self.selection()].iter().collect()
    }

    /// Types `text` over the selection, as much of it as a name takes. Returns
    /// whether that changed anything.
    pub fn insert(&mut self, text: &str) -> bool {
        let selection = self.selection();
        let room = MAX - (self.chars.len() - selection.len());
        let typed: Vec<char> = text.chars().filter_map(typed).take(room).collect();
        if typed.is_empty() {
            return false;
        }
        let end = selection.start + typed.len();
        self.chars.splice(selection, typed);
        (self.caret, self.anchor) = (end, end);
        true
    }

    /// Erases the selection or, without one, the character or the word, if `word`,
    /// after the caret, if `forward`, or before it. Returns whether that changed
    /// anything.
    pub fn erase(&mut self, forward: bool, word: bool) -> bool {
        let mut selection = self.selection();
        if selection.is_empty() {
            let to = self.next(forward, word);
            selection = to.min(self.caret)..to.max(self.caret);
        }
        if selection.is_empty() {
            return false;
        }
        (self.caret, self.anchor) = (selection.start, selection.start);
        self.chars.drain(selection);
        true
    }

    /// Moves the caret over a character, or a word if `word`, forward or back,
    /// selecting what it passes if `select`. Otherwise a selection gives way to the
    /// caret at its edge. Returns whether the caret or the selection moved.
    pub fn step(&mut self, forward: bool, word: bool, select: bool) -> bool {
        let selection = self.selection();
        let to = match (select || word || selection.is_empty(), forward) {
            (true, _) => self.next(forward, word),
            (false, true) => selection.end,
            (false, false) => selection.start,
        };
        self.go(to, select)
    }

    /// Moves the caret before the character `index`, selecting from where the
    /// selection started if `select`. Returns whether the caret or the selection
    /// moved.
    pub fn go(&mut self, index: usize, select: bool) -> bool {
        let index = index.min(self.chars.len());
        let anchor = if select { self.anchor } else { index };
        let moved = (index, anchor) != (self.caret, self.anchor);
        (self.caret, self.anchor) = (index, anchor);
        moved
    }

    pub fn select_all(&mut self) -> bool {
        let all = (self.chars.len(), 0);
        let moved = (self.caret, self.anchor) != all;
        (self.caret, self.anchor) = all;
        moved
    }

    /// Where the character `index` starts as Windows counts text, in UTF-16 units.
    pub fn units(&self, index: usize) -> usize {
        self.chars[..index].iter().map(|c| c.len_utf16()).sum()
    }

    /// The character that starts at `units` UTF-16 units, or the one they end in.
    pub fn index(&self, units: usize) -> usize {
        let mut counted = 0;
        self.chars
            .iter()
            .take_while(|c| {
                counted += c.len_utf16();
                counted <= units
            })
            .count()
    }

    /// Where the caret lands after a character, or a word if `word`, forward or back.
    /// Forward, a word ends where the next one starts; back, at its own start.
    fn next(&self, forward: bool, word: bool) -> usize {
        let at = self.caret;
        let letter = |c: &&char| c.is_alphanumeric();
        let gap = |c: &&char| !c.is_alphanumeric();
        match (forward, word) {
            (true, false) => (at + 1).min(self.chars.len()),
            (false, false) => at.saturating_sub(1),
            (true, true) => {
                let rest = &self.chars[at..];
                let word = rest.iter().take_while(letter).count();
                at + word + rest[word..].iter().take_while(gap).count()
            }
            (false, true) => {
                let before = &self.chars[..at];
                let gaps = before.iter().rev().take_while(gap).count();
                let word = before[..at - gaps].iter().rev().take_while(letter).count();
                at - gaps - word
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAKEN: &str = "2026-11-03 14.32.05";

    #[test]
    fn names_follow_the_date() {
        assert_eq!(prefix(TAKEN), "2026-11-03 - ");
        assert_eq!(stem(TAKEN, "Invoice"), "2026-11-03 - Invoice");
        assert_eq!(name(TAKEN, "2026-11-03 - Invoice"), "Invoice");
        assert_eq!(name(TAKEN, "2026-11-03 - Invoice (2)"), "Invoice (2)");
    }

    #[test]
    fn an_empty_name_brings_back_the_time() {
        assert_eq!(stem(TAKEN, ""), TAKEN);
        assert_eq!(stem(TAKEN, " . "), TAKEN);
        assert_eq!(name(TAKEN, TAKEN), "");
        assert_eq!(name(TAKEN, "2026-11-03 14.32.05 (2)"), "");
    }

    #[test]
    fn names_lose_what_windows_drops() {
        assert_eq!(stem(TAKEN, "  Invoice. . "), "2026-11-03 - Invoice");
        assert_eq!(stem(TAKEN, "v1.2"), "2026-11-03 - v1.2");
    }

    #[test]
    fn typing_keeps_names_valid() {
        assert_eq!(typed('a'), Some('a'));
        assert_eq!(typed('ñ'), Some('ñ'));
        assert_eq!(typed(' '), Some(' '));
        assert_eq!(typed('/'), Some('-'));
        assert_eq!(typed(':'), Some('-'));
        assert_eq!(typed('?'), None);
        assert_eq!(typed('\r'), None);
        assert_eq!(typed('\u{7f}'), None);
    }

    #[test]
    fn typing_replaces_the_selection() {
        let mut field = Field::new("Invoice");
        assert_eq!(field.selected(), "Invoice");
        assert!(field.insert("Re"));
        assert!(field.insert("ceipt"));
        assert_eq!((field.text(), field.caret()), ("Receipt".into(), 7));
        assert!(!field.insert("?*"));
        assert!(field.insert("a/b"));
        assert_eq!(field.text(), "Receipta-b");
    }

    #[test]
    fn names_stop_at_the_most_characters() {
        let mut field = Field::new("");
        field.insert(&"x".repeat(MAX + 5));
        assert_eq!(field.len(), MAX);
        assert!(!field.insert("y"));
        field.go(0, false);
        field.go(3, true);
        assert!(field.insert("abcd"));
        assert_eq!((field.len(), &field.text()[..4]), (MAX, "abcx"));
    }

    #[test]
    fn the_caret_moves_by_characters_and_words() {
        let mut field = Field::new("Step one, two");
        // A selection gives way to the caret at its edge.
        assert!(field.step(false, false, false));
        assert_eq!(field.caret(), 0);
        field.step(true, true, false);
        assert_eq!(field.caret(), 5);
        field.step(true, true, false);
        assert_eq!(field.caret(), 10);
        field.step(false, true, false);
        assert_eq!(field.caret(), 5);
        field.step(true, false, true);
        field.step(true, false, true);
        assert_eq!(field.selected(), "on");
        assert!(field.go(13, false));
        assert!(!field.step(true, false, false));
    }

    #[test]
    fn erasing_takes_characters_words_or_the_selection() {
        let mut field = Field::new("Step one, two");
        field.go(13, false);
        assert!(field.erase(false, false));
        assert_eq!(field.text(), "Step one, tw");
        assert!(field.erase(false, true));
        assert_eq!(field.text(), "Step one, ");
        assert!(field.erase(false, true));
        assert_eq!(field.text(), "Step ");
        field.go(0, false);
        assert!(field.erase(true, true));
        assert_eq!(field.text(), "");
        assert!(!field.erase(false, false));
        let mut field = Field::new("abc");
        field.go(1, false);
        field.go(2, true);
        assert!(field.erase(true, true));
        assert_eq!((field.text(), field.caret()), ("ac".into(), 1));
    }

    #[test]
    fn select_all_selects_everything() {
        let mut field = Field::new("abc");
        assert!(!field.select_all());
        field.go(1, false);
        assert!(field.select_all());
        assert_eq!(field.selected(), "abc");
    }

    #[test]
    fn positions_count_as_windows_does() {
        let field = Field::new("a😀b");
        assert_eq!(field.units(2), 3);
        assert_eq!(field.units(3), 4);
        assert_eq!(field.index(3), 2);
        assert_eq!(field.index(2), 1);
        assert_eq!(field.index(9), 3);
    }
}
