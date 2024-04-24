use serde::{Deserialize, Serialize};
use std::str::Utf8Error;
use thiserror::Error;
use tracing::{debug, trace};

pub trait Chunker {
    fn chunk<'a>(&self, input: &'a str) -> Result<Vec<Chunk<'a>>, ChunkerError>;
}

#[derive(Debug, Error)]
pub enum ChunkerError {
    #[error("{0}")]
    Config(String),

    #[error("utf-8: {0}")]
    Utf8(#[from] Utf8Error),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Chunk<'a> {
    pub content: &'a str,
}

impl<'a> Chunk<'a> {
    pub fn new(content: &'a str) -> Self {
        Chunk { content }
    }
}

/// Default chunk size for all chunkers
const DEFAULT_SIZE: usize = 1000;

/// Default chunk overlap for all character based chunkers
const DEFAULT_OVERLAP: usize = 500;

/// Default delimiters for the [recursive chunker][Recursive].
const DEFAULT_DELIMS: &[&str] = &["\n\n", "\n", " ", ""];

/// Default delimiters for the [recursive chunker][Recursive] when constructed with
/// [Recursive::markdown].
const MARKDOWN_DELIMS: &[&str] = &[
    "#", "##", "###", "####", "#####", "######", "\n```", "\n---\n", "\n___\n", "\n\n", "\n", " ",
    "",
];

#[derive(Debug)]
pub struct SlidingWindow {
    /// Base chunk size.
    size: usize,

    /// The overlap per chunk.
    overlap: usize,
}

impl SlidingWindow {
    pub fn new(size: usize, overlap: usize) -> Result<Self, ChunkerError> {
        if overlap >= size {
            return Err(ChunkerError::Config(format!(
                "size ({size}) must be greater than overlap ({overlap})"
            )));
        }

        Ok(Self { size, overlap })
    }
}

impl Chunker for SlidingWindow {
    fn chunk<'a>(&self, input: &'a str) -> Result<Vec<Chunk<'a>>, ChunkerError> {
        let SlidingWindow { size, overlap } = self;

        let input = input.trim();

        if input.is_empty() {
            return Ok(vec![]);
        }

        if input.len() <= size + overlap {
            return Ok(vec![Chunk::new(input)]);
        }

        let mut chunks = vec![];

        let mut start = 0;
        let mut end = *size;
        let input_size = input.len();

        loop {
            let chunk_start = if start == 0 { 0 } else { start - overlap };
            let chunk_end = end + overlap;

            if chunk_end > input_size {
                let chunk = Chunk::new(&input[chunk_start..input_size]);
                chunks.push(chunk);
                break;
            }

            let chunk = Chunk::new(&input[chunk_start..chunk_end]);
            trace!("Chunked: {:?}\n", chunk.content);
            chunks.push(chunk);

            start = end;
            end += size;
        }

        debug!(
            "Chunked {} chunks, avg chunk size: {}",
            chunks.len(),
            chunks.iter().fold(0, |acc, el| acc + el.content.len()) / chunks.len()
        );

        Ok(chunks)
    }
}

impl Default for SlidingWindow {
    fn default() -> Self {
        Self {
            size: DEFAULT_SIZE,
            overlap: DEFAULT_OVERLAP,
        }
    }
}

/// A chunker based on langchain's
/// [RecursiveCharacterSplitter](https://dev.to/eteimz/understanding-langchains-recursivecharactertextsplitter-2846).
///
/// Given a default size and set of delimiters, recursively splits the input using the delimiters.
///
/// The default `delims` are : `["\n\n", "\n", " ", ""]`.
///
/// The input is first split into chunks with the first delimiter. For each chunk larger than `size`, split
/// it with the next delimiter in the chain until small enough chunks can be assembled.
#[derive(Debug)]
pub struct Recursive<'a> {
    /// Target chunk size.
    pub size: usize,

    /// Chunk overlap.
    pub overlap: usize,

    /// The delimiters to use when splitting.
    pub delims: &'a [&'a str],
}

impl<'delim> Recursive<'delim> {
    pub fn new(size: usize, overlap: usize, delimiters: &'delim [&str]) -> Self {
        Recursive {
            size,
            overlap,
            delims: delimiters,
        }
    }

    pub fn chunk_size(mut self, size: usize) -> Self {
        self.size = size;
        self
    }

    pub fn overlap(mut self, overlap: usize) -> Self {
        self.overlap = overlap;
        self
    }

    pub fn delimiters(mut self, delimiters: &'delim [&str]) -> Self {
        self.delims = delimiters;
        self
    }

    pub fn markdown() -> Self {
        Self {
            size: DEFAULT_SIZE,
            overlap: DEFAULT_OVERLAP,
            delims: MARKDOWN_DELIMS,
        }
    }

    /// Chunk the input using this instance's delimiters.
    ///
    /// `input` - The input text to chunk
    ///
    /// `idx` - The current delimiter index.
    ///
    /// `buffer` - A slice in which the current split contents are stored if they are smaller than
    /// this instance's `size'. Must live at least as long as the input.
    ///
    /// `chunks` - Where the final chunks are stored.
    ///
    /// The function initially splits `input` with `delims[idx]`. For each split larger than `size`,
    /// another round of splitting is performed using the next delimiter in `delims`.
    /// In each round, the buffer contents are populated until the next chunk
    /// would cause it to be larger than `size`. When this happens, the current buffer is pushed
    /// into `chunks`.
    ///
    /// Since the buffer is shared between rounds, chunks from the next round's
    /// split will be included in it, maximising the amount of content in the chunk.
    ///
    /// If the chunk is of greater size than allowed and no more delimiters are left,
    /// the chunk will be skipped.
    fn chunk_recursive<'input>(
        &self,
        input: &'input str,
        idx: usize,
        mut buffer: &'input str,
        chunks: &mut Vec<&'input str>,
    ) -> Result<Option<&'input str>, ChunkerError> {
        let Recursive { size, delims, .. } = self;

        if idx >= delims.len() {
            return Ok(None);
        }

        let split: std::str::SplitInclusive<'input, &str> = input.split_inclusive(delims[idx]);

        for chunk in split {
            if buffer.len() + chunk.len() <= *size {
                // Buffer is shared through invocations so use it if not empty
                let buf = if buffer.is_empty() {
                    chunk.as_ptr()
                } else {
                    buffer.as_ptr()
                };

                let buf = std::ptr::slice_from_raw_parts(buf, buffer.len() + chunk.len());

                // SAFETY: We know we are always pointing to something of lifetime 'input
                // and that it lives through each invocation. We are always incrementing
                // the pointer by the chunk length so we are never out of bounds.
                unsafe {
                    buffer = std::str::from_utf8(&*buf)?;
                }

                continue;
            }

            // Can't store current chunk with existing buf
            // If the buf is not empty, add it to the chunks and reset buffer
            if !buffer.is_empty() {
                chunks.push(buffer);

                // Check again and reset loop if it fits, setting the current buffer
                // to the chunk
                if chunk.len() <= *size {
                    buffer = chunk;
                    continue;
                }

                // Otherwise just reset the buffer and do another round
                buffer = "";
            }

            if let Some(buf) = self.chunk_recursive(chunk, idx + 1, buffer, chunks)? {
                buffer = buf;
            }
        }

        // If there's still something at the end of the fn return it
        if !buffer.is_empty() {
            return Ok(Some(buffer));
        }

        Ok(None)
    }
}

impl<'delim> Chunker for Recursive<'delim> {
    fn chunk<'input>(&self, input: &'input str) -> Result<Vec<Chunk<'input>>, ChunkerError> {
        let mut splits = vec![];
        if let Some(split) = self.chunk_recursive(input, 0, "", &mut splits)? {
            splits.push(split);
        }

        let mut chunks = vec![];

        for i in 0..splits.len() {
            let current = splits[i];

            if i == 0 && splits.len() == 1 {
                chunks.push(current);
                break;
            }

            // Special case for first item
            if i == 0 {
                let next = splits[i + 1];
                if next.len() <= self.overlap {
                    let chunk = combine_str(current, next)?;
                    chunks.push(chunk);
                } else {
                    let chunk = combine_str(current, &next[..=self.overlap])?;
                    chunks.push(chunk);
                }
                continue;
            }

            // Special case for last item
            if i == splits.len() - 1 {
                let prev = &splits[i - 1];
                if prev.len() <= self.overlap {
                    let chunk = combine_str(prev, current)?;
                    chunks.push(chunk);
                } else {
                    let chunk = combine_str(&prev[self.overlap..], current)?;
                    chunks.push(chunk);
                }
                break;
            }

            let prev = &splits[i - 1];
            let prev = if prev.len() <= self.overlap {
                prev
            } else {
                &prev[self.overlap..]
            };

            let next = splits[i + 1];
            let next = if next.len() <= self.overlap {
                next
            } else {
                &next[..self.overlap]
            };

            let current = combine_str(prev, current)?;
            let chunk = combine_str(current, next)?;

            chunks.push(chunk);
        }

        println!(
            "Chunked {} chunks, avg chunk size: {}",
            chunks.len(),
            if chunks.is_empty() {
                0
            } else {
                chunks.iter().fold(0, |acc, el| acc + el.len()) / chunks.len()
            }
        );

        Ok(chunks
            .into_iter()
            .filter_map(|chunk| (!chunk.trim().is_empty()).then_some(Chunk::new(chunk)))
            .collect())
    }
}

impl Default for Recursive<'_> {
    fn default() -> Self {
        Self {
            size: DEFAULT_SIZE,
            overlap: DEFAULT_OVERLAP,
            delims: DEFAULT_DELIMS,
        }
    }
}

#[derive(Debug)]
pub struct SlidingWindowDelimited {
    /// Base chunk size. Cannot be exact in this case since the chunks are based on sentences
    /// which are of arbitrary length.
    pub size: usize,

    /// The amount of sentences that will be present in the current chunk from the chunk prior and
    /// chunk after.
    pub overlap: usize,

    /// The delimiter to use to split sentences. At time of writing the most common one is ".".
    pub delimiter: char,

    pub skip: &'static [&'static str],
}

#[derive(Debug)]
struct Cursor<'a> {
    buf: &'a str,
    offset: usize,
    delim: char,
}

impl<'a> Cursor<'a> {
    fn new(input: &'a str, delim: char) -> Self {
        Self {
            buf: input,
            offset: 0,
            delim,
        }
    }

    fn get_slice(&self) -> &'a str {
        if self.buf.is_empty() {
            self.buf
        } else {
            &self.buf[..self.offset]
        }
    }

    /// Advance the offset until `delim` is found. The offset will be set
    /// to the index following the delim.
    fn advance(&mut self) {
        if self.buf.is_empty() || self.offset == self.buf.len() - 1 {
            return;
        }

        let mut chars = self.buf.chars().skip(self.offset);

        for ch in chars.by_ref() {
            self.offset += 1;
            if ch == self.delim {
                break;
            }
        }

        while chars.next().is_some_and(|ch| ch == self.delim) {
            self.offset += 1;
        }
    }

    /// Returns `true` if the cursor is finished.
    fn advance_exact(&mut self, amt: usize) -> bool {
        if self.offset + amt >= self.buf.len() {
            self.offset += self.offset + amt - self.buf.len();
            return true;
        }
        self.offset += amt;
        self.offset == self.buf.len() - 1
    }

    fn peek(&self, pat: &str) -> bool {
        if self.offset.saturating_sub(pat.len()) == 0 {
            return false;
        }
        &self.buf[self.offset - pat.len()..self.offset] == pat
    }

    fn finished(&self) -> bool {
        self.offset == self.buf.len() - 1
    }
}

#[derive(Debug)]
struct CursorRev<'a> {
    buf: &'a str,
    offset: usize,
    delim: char,
}

impl<'a> CursorRev<'a> {
    fn new(input: &'a str, delim: char) -> Self {
        Self {
            buf: input,
            offset: input.len().saturating_sub(1),
            delim,
        }
    }

    fn get_slice(&self) -> &'a str {
        if self.offset == 0 {
            self.buf
        } else {
            &self.buf[self.offset + 1..]
        }
    }

    fn advance(&mut self) {
        if self.offset == 0 {
            return;
        }
        self.offset -= 1;

        let mut chars = self
            .buf
            .chars()
            .rev()
            .skip(self.buf.len() - 1 - self.offset);

        loop {
            let Some(ch) = chars.next() else {
                break;
            };

            if ch == self.delim {
                // Advance until end of delimiter sequence
                while chars.next().is_some_and(|ch| ch == self.delim) {
                    self.offset -= 1;
                }

                break;
            }

            self.offset -= 1;

            if self.offset == 0 {
                break;
            }
        }
    }

    /// Returns `true` if the cursor is finished.
    fn advance_exact(&mut self, amt: usize) -> bool {
        self.offset = self.offset.saturating_sub(amt);
        self.offset == 0
    }

    fn peek_back(&self, pat: &str) -> bool {
        if self.offset.saturating_sub(pat.len()) == 0 {
            return false;
        }
        &self.buf[self.offset - pat.len()..self.offset] == pat
    }
}

impl Chunker for SlidingWindowDelimited {
    fn chunk<'a>(&self, input: &'a str) -> Result<Vec<Chunk<'a>>, ChunkerError> {
        let Self {
            size,
            overlap,
            delimiter: delim,
            skip,
        } = self;

        let mut chunks = vec![];

        let mut cursor = Cursor::new(input, *delim);
        let mut chunk = &input[..1];
        let mut start = 1;

        'main: loop {
            if start >= input.len() {
                if !chunk.is_empty() {
                    chunks.push(Chunk::new(chunk))
                }
                break;
            }

            // Advance until delim
            cursor.advance();

            if !cursor.finished() {
                for s in *skip {
                    if cursor.peek(s) {
                        cursor.advance_exact(s.len());
                        continue 'main;
                    }
                }
            }

            let piece = &input[start..cursor.offset];
            chunk = combine_str(chunk, piece)?;
            start += piece.len();

            if chunk.len() >= *size {
                let prev = &input[..cursor.offset - chunk.len()];
                let next = &input[cursor.offset..];

                let mut p_cursor = CursorRev::new(prev, *delim);
                let mut n_cursor = Cursor::new(next, *delim);

                for _ in 0..*overlap {
                    p_cursor.advance();
                    for s in *skip {
                        if p_cursor.peek_back(s) {
                            p_cursor.advance_exact(s.len());
                            continue;
                        }
                    }
                    n_cursor.advance();
                    for s in *skip {
                        if n_cursor.peek(s) {
                            n_cursor.advance_exact(s.len());
                            continue;
                        }
                    }
                }

                let prev = p_cursor.get_slice();
                let next = n_cursor.get_slice();

                let chunk_full = combine_str(combine_str(prev, chunk)?, next)?;

                chunks.push(Chunk::new(chunk_full));

                if start + 1 >= input.len() {
                    break;
                }
                chunk = &input[start..start + 1];
            }
        }

        Ok(chunks)
    }
}

impl Default for SlidingWindowDelimited {
    fn default() -> Self {
        Self {
            size: DEFAULT_SIZE,
            overlap: 5,
            delimiter: '.',
            skip: &[],
        }
    }
}

#[inline]
fn combine_str<'a>(start_str: &'a str, end_str: &'a str) -> Result<&'a str, Utf8Error> {
    let current_ptr =
        std::ptr::slice_from_raw_parts(start_str.as_ptr(), start_str.len() + end_str.len());
    unsafe { std::str::from_utf8(&*current_ptr) }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn pointer_sanity() {
        let input = "Hello\nWorld";
        let split = input.split_inclusive('\n').collect::<Vec<_>>();

        let one = split[0];
        let two = split[1];

        let combined = std::ptr::slice_from_raw_parts(one.as_ptr(), one.len() + two.len());
        unsafe {
            assert_eq!(input, std::str::from_utf8(&*combined).unwrap());
        }
    }

    #[test]
    fn sliding_window_works() {
        let input = "Sticks and stones may break my bones, but words will never leverage agile frameworks to provide a robust synopsis for high level overviews.";
        let window = SlidingWindow::new(30, 20).unwrap();
        let chunks = window.chunk(input).unwrap();

        assert_eq!(&input[0..50], chunks[0].content);
        assert_eq!(&input[10..80], chunks[1].content);
        assert_eq!(&input[40..110], chunks[2].content);
        assert_eq!(&input[70..input.len()], chunks[3].content);
    }

    #[test]
    fn sliding_window_empty() {
        let input = "";
        let window = SlidingWindow::new(1, 0).unwrap();
        let chunks = window.chunk(input).unwrap();

        assert!(chunks.is_empty());
    }

    #[test]
    fn sliding_window_small_input() {
        let input = "Foobar";
        let window = SlidingWindow::new(30, 20).unwrap();
        let chunks = window.chunk(input).unwrap();

        assert_eq!(input, chunks[0].content);
    }

    const INPUT: &str = r#"
What I Worked On

February 2021

Before college the two main things I worked on, outside of school, were writing and programming. I didn't write essays. I wrote what beginning writers were supposed to write then, and probably still are: short stories. My stories were awful. They had hardly any plot... just characters with strong feelings, which I imagined made them deep.

The first programs I tried writing were on the IBM 1401 that our school district used for what was then called "data processing." This was in 9th grade, so I was 13 or 14. The school district's 1401 happened to be in the basement of our junior high school, and my friend Rich Draves and I got permission to use it. It was like a mini Bond villain's lair down there, with all these alien-looking machines — CPU, disk drives, printer, card reader — sitting up on a raised floor under bright fluorescent lights.
"#;

    #[test]
    fn recursive_works() {
        let chunker = Recursive::new(100, 50, DEFAULT_DELIMS);
        let mut chunks = vec![];

        chunker
            .chunk_recursive(INPUT.trim(), 0, "", &mut chunks)
            .unwrap();

        for chunk in chunks {
            assert!(chunk.len() <= 100);
        }
    }

    #[test]
    fn recursive_small_input_custom_delims() {
        let input = "Supercalifragilisticexpialadocius";
        let chunker = Recursive::new(5, 0, &["foo"]);
        let chunks = chunker.chunk(input).unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn sliding_window_delim_works() {
        let chunker = SlidingWindowDelimited {
            size: 50,
            overlap: 2,
            delimiter: '.',
            skip: &[],
        };

        let chunks = chunker.chunk(INPUT.trim()).unwrap();
        for chunk in chunks {
            dbg!(chunk);
        }
    }

    #[test]
    fn sliding_window_delim_small() {
        let input = r#"
Exactly 1.
Exactly 2.
Exactly 3.
Exactly 4.
Exactly 5.
Exactly 6.
Exactly 7.
Exactly 8.
Exactly 9.
Exactly 10.
Exactly 11.
Exactly 12.
Exactly 13.
Exactly 14.
Exactly 15.
Exactly 16.
Exactly 17.
        "#;
        let chunker = SlidingWindowDelimited {
            size: 10,
            overlap: 2,
            delimiter: '.',
            skip: &[],
        };

        let chunks = chunker.chunk(input.trim()).unwrap();
        for chunk in chunks {
            dbg!(chunk);
        }
    }

    #[test]
    fn sliding_window_delim_skip() {
        let input = r#"Skip this........ Please do so. It would... Be a shame if you didn't."#;

        let chunker = SlidingWindowDelimited {
            size: 10,
            overlap: 1,
            delimiter: '.',
            skip: &[],
        };

        let chunks = chunker.chunk(input.trim()).unwrap();
        for chunk in chunks {
            dbg!(chunk);
        }
    }

    #[test]
    fn recursive_works_with_file() {
        let input = std::fs::read_to_string("content/README.md").unwrap();

        let chunker = Recursive {
            delims: &[
                "######", "#####", "####", "###", "##", "#", "```", "\n---\n", "\n___\n", "\n\n",
                "\n", " ", "",
            ],
            ..Default::default()
        };

        let chunks = chunker.chunk(input.trim()).unwrap();
        for chunk in chunks {
            dbg!(chunk);
        }
    }

    #[test]
    fn swd_works_with_file() {
        let input = std::fs::read_to_string("content/README.md").unwrap();

        let chunker = SlidingWindowDelimited::default();

        let chunks = chunker.chunk(input.trim()).unwrap();
        for chunk in chunks {
            dbg!(chunk);
        }
    }
}
