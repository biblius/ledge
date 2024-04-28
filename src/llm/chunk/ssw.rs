use super::{concat, Chunk, Chunker, ChunkerError, DEFAULT_SIZE};

/// Heuristic chunker for texts intended for humans, e.g. documentation, books, blogs, etc.
///
/// Basically a sliding window which is aware of sentence stops, currently the only stop
/// implemented is the '.' character.
///
/// It will attempt to chunk the content according to `size`. Keep in mind it cannot
/// be exact and the chunks will probably be larger, because of the way it searches
/// for delimiters.
///
/// The chunker can also be configured to skip common patterns including the fullstop
/// such as abbreviations (e.g., i.e., etc.) and urls.
#[derive(Debug)]
pub struct SnappingSlidingWindow<'skip> {
    /// Base chunk size. Cannot be exact in this case since the chunks are based on sentences
    /// which are of arbitrary length.
    size: usize,

    /// The amount of sentences that will be present in the current chunk from the chunk prior and
    /// chunk after.
    overlap: usize,

    /// The delimiter to use to split sentences. At time of writing the most common one is ".".
    delimiter: char,

    /// Whenever a delimiter is found, the chunker will look ahead for these sequences
    /// and will skip the delimiter if found, basically treating it as a regular char.
    ///
    /// Useful for common abbreviations and urls.
    skip_forward: &'skip [&'skip str],

    /// Whenever a delimiter is found, the chunker will look back for these sequences
    /// and will skip the delimiter if found, basically treating it as a regular char.
    ///
    /// Useful for common abbreviations and urls.
    skip_back: &'skip [&'skip str],
}

impl<'skip> SnappingSlidingWindow<'skip> {
    pub fn new(size: usize, overlap: usize) -> Self {
        Self {
            size,
            overlap,
            ..Default::default()
        }
    }

    pub fn delimiter(mut self, delimiter: char) -> Self {
        self.delimiter = delimiter;
        self
    }

    pub fn skip_forward(mut self, skip_forward: &'skip [&'skip str]) -> Self {
        self.skip_forward = skip_forward;
        self
    }

    pub fn skip_back(mut self, skip_back: &'skip [&'skip str]) -> Self {
        self.skip_back = skip_back;
        self
    }
}

impl<'skip> Chunker for SnappingSlidingWindow<'skip> {
    fn chunk<'a>(&self, input: &'a str) -> Result<Vec<Chunk<'a>>, ChunkerError> {
        let Self {
            size,
            overlap,
            delimiter: delim,
            skip_forward,
            skip_back,
        } = self;

        let mut chunks = vec![];

        let mut cursor = Cursor::new(input, *delim);
        let mut chunk = &input[..1];
        let mut start = 1;

        loop {
            if start >= input.len() {
                if !chunk.is_empty() {
                    chunks.push(Chunk::new(chunk))
                }
                break;
            }

            // Advance until delim
            cursor.advance();

            if cursor.advance_if_peek(skip_forward, skip_back) {
                continue;
            }

            let piece = &input[start..cursor.pos];

            chunk = concat(chunk, piece)?;
            start += piece.len();

            if chunk.len() < *size {
                continue;
            }

            let prev = &input[..cursor.pos - chunk.len()];
            let next = &input[cursor.pos..];

            let mut p_cursor = CursorRev::new(prev, *delim);
            let mut n_cursor = Cursor::new(next, *delim);

            for _ in 0..*overlap {
                loop {
                    p_cursor.advance();
                    if !p_cursor.advance_if_peek(skip_forward, skip_back) {
                        break;
                    }
                }

                loop {
                    n_cursor.advance();
                    if !n_cursor.advance_if_peek(skip_forward, skip_back) {
                        break;
                    }
                }
            }

            let prev = p_cursor.get_slice();
            let next = n_cursor.get_slice();

            let chunk_full = concat(concat(prev, chunk)?, next)?;

            chunks.push(Chunk::new(chunk_full));

            start += 1;

            if start + n_cursor.pos >= input.len() {
                break;
            }

            chunk = &input[start - 1..start];
        }

        Ok(chunks)
    }
}

impl Default for SnappingSlidingWindow<'_> {
    fn default() -> Self {
        Self {
            size: DEFAULT_SIZE,
            overlap: 5,
            delimiter: '.',
            // Common urls, abbreviations, file extensions
            skip_forward: &["com", "org", "net", "g.", "e.", "sh", "rs", "js", "json"],
            skip_back: &["www", "etc", "e.g", "i.e"],
        }
    }
}

#[derive(Debug)]
struct Cursor<'a> {
    buf: &'a str,
    pos: usize,
    delim: char,
}

impl<'a> Cursor<'a> {
    fn new(input: &'a str, delim: char) -> Self {
        Self {
            buf: input,
            pos: 0,
            delim,
        }
    }

    fn get_slice(&self) -> &'a str {
        if self.buf.is_empty() || self.pos == self.buf.len() - 1 {
            return self.buf;
        }
        &self.buf[..self.pos]
    }

    /// Advance the pos until `delim` is found. The pos will be set
    /// to the index following the delim.
    fn advance(&mut self) {
        if self.buf.is_empty() || self.pos == self.buf.len() - 1 {
            return;
        }

        let mut chars = self.buf.chars().skip(self.pos);

        loop {
            let Some(ch) = chars.next() else {
                debug_assert!(self.pos == self.buf.len() - 1);
                break;
            };

            self.pos += 1;

            if self.pos == self.buf.len() - 1 {
                break;
            }

            if ch == self.delim {
                let mut stop = true;

                while chars.next().is_some_and(|ch| ch == self.delim) {
                    self.pos += 1;
                    stop = false;
                }

                if stop {
                    break;
                }

                self.pos += 1;
            }
        }
    }

    /// Returns `true` if the cursor is finished.
    fn advance_exact(&mut self, amt: usize) {
        if self.pos + amt >= self.buf.len() {
            self.pos = self.buf.len() - 1;
        }
        self.pos += amt;
    }

    fn peek_back(&self, pat: &str) -> bool {
        if self.pos.saturating_sub(pat.len()) == 0 {
            return false;
        }

        // pos is always advanced past delimiter unless it is at the end of buf
        if self.pos == self.buf.len() - 1 {
            // TODO
            if &self.buf[self.pos..] == "." {
                return &self.buf[self.pos - pat.len()..self.pos] == pat;
            }
            return &self.buf[self.pos - pat.len()..=self.pos] == pat;
        }

        &self.buf[self.pos - 1 - pat.len()..self.pos - 1] == pat
    }

    fn peek_forward(&self, pat: &str) -> bool {
        if self.pos + pat.len() >= self.buf.len() {
            return false;
        }
        &self.buf[self.pos..self.pos + pat.len()] == pat
    }

    fn advance_if_peek(&mut self, forward: &[&str], back: &[&str]) -> bool {
        for s in back {
            if self.peek_back(s) {
                return true;
            }
        }

        for s in forward {
            if self.peek_forward(s) {
                self.advance_exact(s.len());
                return true;
            }
        }

        false
    }
}

/// Cursor for scanning a string backwards. The `pos` of this cursor is always
/// kept at `delim` points in `buf`.
#[derive(Debug)]
struct CursorRev<'a> {
    /// The str being scanned.
    buf: &'a str,

    /// The current byte position of the cursor in the str.
    pos: usize,

    /// The delimiter to snap to
    delim: char,
}

impl<'a> CursorRev<'a> {
    fn new(input: &'a str, delim: char) -> Self {
        Self {
            buf: input,
            pos: input.len().saturating_sub(1),
            delim,
        }
    }

    fn get_slice(&self) -> &'a str {
        if self.pos == 0 {
            self.buf
        } else {
            &self.buf[self.pos + 1..]
        }
    }

    fn advance(&mut self) {
        if self.pos == 0 {
            return;
        }

        let mut chars = self.buf.chars().rev().skip(self.buf.len() - 1 - self.pos);

        let mut first_iter = true;
        loop {
            let Some(ch) = chars.next() else {
                debug_assert!(self.pos == 0);
                break;
            };

            self.pos -= 1;

            if self.pos == 0 {
                break;
            }

            if ch == self.delim {
                let mut stop = true;

                // Advance until end of delimiter sequence
                while chars.next().is_some_and(|ch| ch == self.delim) {
                    self.pos -= 1;
                    stop = false;
                }

                // We've invoked next on the chars and have to adjust
                // We don't have to increment pos when we're stopping
                // since there's no delim and we're breaking anyway
                if !stop {
                    self.pos -= 1;
                }

                if stop && !first_iter {
                    break;
                }

                first_iter = false;
            }
        }
    }

    /// Returns `true` if the cursor is finished.
    fn advance_exact(&mut self, amt: usize) -> bool {
        self.pos = self.pos.saturating_sub(amt);
        self.pos == 0
    }

    fn peek_back(&self, pat: &str) -> bool {
        &self.buf[self.pos.saturating_sub(pat.len())..self.pos] == pat
    }

    fn peek_forward(&self, pat: &str) -> bool {
        if self.pos + pat.len() >= self.buf.len() {
            return false;
        }
        let start = if self.pos == 0 { 0 } else { self.pos + 1 };
        let mut end = self.pos + pat.len();
        if self.pos > 0 {
            end += 1;
        }
        &self.buf[start..end] == pat
    }

    fn advance_if_peek(&mut self, forward: &[&str], back: &[&str]) -> bool {
        for s in back {
            if self.peek_back(s) {
                self.advance_exact(s.len());
                return true;
            }
        }

        for s in forward {
            if self.peek_forward(s) {
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_size() {
        let ch = 'Ü';
        let mut bytes = [0, 0];
        assert_eq!(2, ch.encode_utf8(&mut bytes).len());
    }

    #[test]
    fn constructor() {
        // For lifetime sanity checks
        let skip_f = vec![String::from("foo"), String::from("bar")];
        let skip_f: Vec<_> = skip_f.iter().map(|s| s.as_str()).collect();

        let skip_b = vec![String::from("foo"), String::from("bar")];
        let skip_b: Vec<_> = skip_b.iter().map(|s| s.as_str()).collect();
        let size = 1;
        let overlap = 1;
        let delimiter = '!';

        let chunker = SnappingSlidingWindow::new(size, overlap)
            .delimiter(delimiter)
            .skip_forward(&skip_f)
            .skip_back(&skip_b);

        assert_eq!(delimiter, chunker.delimiter);
        assert_eq!(size, chunker.size);
        assert_eq!(overlap, chunker.overlap);
        assert_eq!(&skip_f, chunker.skip_forward);
        assert_eq!(&skip_b, chunker.skip_back);
    }

    #[test]
    fn cursor_advances_to_delimiter() {
        let input = "This is such a sentence. One of the sentences in the world. Super wow.";
        let mut cursor = Cursor::new(input, '.');
        let expected = [
            "This is such a sentence.",
            "This is such a sentence. One of the sentences in the world.",
            input,
        ];
        assert!(cursor.get_slice().is_empty());
        for test in expected {
            cursor.advance();
            assert_eq!(test, cursor.get_slice());
        }
    }

    #[test]
    fn cursor_advances_past_repeating_delimiters() {
        let input = "This is such a sentence... One of the sentences in the world. Super wow.";
        let mut cursor = Cursor::new(input, '.');
        let expected = [
            "This is such a sentence... One of the sentences in the world.",
            input,
        ];
        for test in expected {
            cursor.advance();
            assert_eq!(test, cursor.get_slice());
        }
    }

    #[test]
    fn cursor_advances_exact() {
        let input = "This is Sparta my friend";
        let mut cursor = Cursor::new(input, '.');
        let expected = input.split_inclusive(' ');
        let mut buf = String::new();
        for test in expected {
            assert_eq!(&buf, cursor.get_slice());
            cursor.advance_exact(test.len());
            buf.push_str(test);
        }
    }

    #[test]
    fn cursor_peek_forward() {
        let input = "This. Is. Sentence. etc.";
        let mut cursor = Cursor::new(input, '.');
        let expected = ["This", " Is", " Sentence", " etc"];
        for test in expected {
            assert!(cursor.peek_forward(test));
            cursor.advance();
        }
        assert!(!cursor.peek_forward("etc"));
    }

    #[test]
    fn cursor_peek_back() {
        let input = "This. Is. Sentence. etc.";
        let mut cursor = Cursor::new(input, '.');
        let expected = ["This", " Is", " Sentence", " etc"];
        assert!(!cursor.peek_back("This"));
        for test in expected {
            cursor.advance();
            dbg!(test);
            assert!(cursor.peek_back(test));
        }
        assert!(cursor.peek_back("etc"));
    }

    #[test]
    fn rev_cursor_advances_to_delimiter() {
        let input = "This is such a sentence. One of the sentences in the world. Super wow.";
        let mut cursor = CursorRev::new(input, '.');
        let expected = [
            " Super wow.",
            " One of the sentences in the world. Super wow.",
            input,
        ];
        for test in expected {
            cursor.advance();
            assert_eq!(test, cursor.get_slice());
        }
    }

    #[test]
    fn rev_cursor_advances_past_repeating_delimiters() {
        let input =
            "This is such a sentence..... Very sentencey. So many.......... words. One of the sentences in the world... Super wow.";
        let mut cursor = CursorRev::new(input, '.');
        let expected = [
            " One of the sentences in the world... Super wow.",
            " So many.......... words. One of the sentences in the world... Super wow.",
            input,
        ];
        for test in expected {
            cursor.advance();
            assert_eq!(test, cursor.get_slice());
        }
    }

    #[test]
    fn rev_cursor_advances_exact() {
        let input = "This is Sparta my friend";
        let mut cursor = CursorRev::new(input, '.');
        let mut buf = String::new();
        let expected = input.split_inclusive(' ');
        for test in expected.into_iter().rev() {
            assert_eq!(&buf, cursor.get_slice());
            cursor.advance_exact(test.len());
            buf.insert_str(0, test);
        }
    }

    #[test]
    fn rev_cursor_peek_forward() {
        let input = "This. Is. Sentence. etc.";
        let mut cursor = CursorRev::new(input, '.');
        let expected = ["This", " Is", " Sentence", " etc"];
        for test in expected.into_iter().rev() {
            cursor.advance();
            assert!(cursor.peek_forward(test));
        }
        assert!(cursor.peek_forward("This"));
    }

    #[test]
    fn rev_cursor_peek_back() {
        let input = "This. Is. Sentence. etc.";
        let mut cursor = CursorRev::new(input, '.');
        let expected = ["This", " Is", " Sentence", " etc"];
        assert!(cursor.peek_back("etc"));
        for test in expected.into_iter().rev() {
            assert!(cursor.peek_back(test));
            cursor.advance();
        }
        assert!(!cursor.peek_back("etc"));
    }

    #[test]
    fn ssw_works() {
        let input =
            "I have a sentence. It is not very long. Here is another. Long schlong ding dong.";
        let chunker = SnappingSlidingWindow {
            size: 1,
            overlap: 1,
            ..Default::default()
        };
        let expected = [
            "I have a sentence. It is not very long.",
            "I have a sentence. It is not very long. Here is another.",
            " It is not very long. Here is another. Long schlong ding dong.",
        ];

        let chunks = chunker.chunk(input.trim()).unwrap();
        assert_eq!(3, chunks.len());

        for (chunk, test) in chunks.into_iter().zip(expected.into_iter()) {
            assert_eq!(test, chunk.content);
        }
    }

    #[test]
    fn ssw_skips_back() {
        let input =
            "I have a sentence. It contains letters, words, etc. This one contains more. The most important of which is foobar., because it must be skipped.";
        let chunker = SnappingSlidingWindow {
            size: 1,
            overlap: 1,
            skip_back: &["etc", "foobar"],
            ..Default::default()
        };
        let expected = [
            "I have a sentence. It contains letters, words, etc. This one contains more.",
            input,
        ];

        let chunks = chunker.chunk(input.trim()).unwrap();
        assert_eq!(2, chunks.len());

        for (chunk, test) in chunks.into_iter().zip(expected.into_iter()) {
            assert_eq!(test, chunk.content);
        }
    }

    #[test]
    fn ssw_skips_forward() {
        let input =
            "Go to sentences.org for more words. 50% off on words with >4 syllables. Leverage agile frameworks to provide robust high level overview at agile.com.";
        let chunker = SnappingSlidingWindow {
            size: 1,
            overlap: 1,
            skip_forward: &["com", "org"],
            ..Default::default()
        };
        let expected = [
            "Go to sentences.org for more words. 50% off on words with >4 syllables.",
            input,
        ];

        let chunks = chunker.chunk(input.trim()).unwrap();
        assert_eq!(2, chunks.len());

        for (chunk, test) in chunks.into_iter().zip(expected.into_iter()) {
            assert_eq!(test, chunk.content);
        }
    }

    #[test]
    fn ssw_skips_common_abbreviations() {
        let input =
            "Words are hard. There are many words in existence, e.g. this, that, etc..., quite a few, as you can see. My opinion, available at nobodycares.com, is that words should convey meaning. Not everyone agrees however, which is why they leverage agile frameworks to provide synopsises for high level overview, i.e. they speak nonsense.";

        let chunker = SnappingSlidingWindow {
            size: 1,
            overlap: 1,
            ..Default::default()
        };

        let expected = [
            "Words are hard. There are many words in existence, e.g. this, that, etc..., quite a few, as you can see.",
            "Words are hard. There are many words in existence, e.g. this, that, etc..., quite a few, as you can see. My opinion, available at nobodycares.com, is that words should convey meaning.",
            input
        ];

        let chunks = chunker.chunk(input.trim()).unwrap();
        // assert_eq!(3, chunks.len());

        for (chunk, test) in chunks.into_iter().zip(expected.into_iter()) {
            assert_eq!(test, chunk.content);
        }
    }
}
