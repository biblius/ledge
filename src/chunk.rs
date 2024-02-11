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

#[derive(Debug)]
pub struct Chunk<'a> {
    pub content: &'a str,
}

impl<'a> Chunk<'a> {
    pub fn new(content: &'a str) -> Self {
        Chunk { content }
    }
}

/// Default chunk size for all chunkers
const DEFAULT_SIZE: usize = 500;

/// Default chunk overlap for all chunkers
const DEFAULT_OVERLAP: usize = 200;

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

const DEFAULT_DELIMS: &[&str] = &["\n\n", "\n", " ", ""];

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
    /// another round of splitting is performed using the next delimiter in `delims`, i.e.
    /// `delims[idx + 1]`. In each round, the buffer contents are populated until the next chunk
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
                    buffer = &*std::str::from_utf8(&*buf)?;
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
                    buffer = &*chunk;
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

        fn combine_str<'a>(start_str: &'a str, end_str: &'a str) -> Result<&'a str, Utf8Error> {
            let current_ptr =
                std::ptr::slice_from_raw_parts(start_str.as_ptr(), start_str.len() + end_str.len());

            // SAFETY: We know we're withing the bounds of the original string since
            // every split perfectly aligns with the next and previous one
            Ok(unsafe { std::str::from_utf8(&*current_ptr) }?)
        }

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

            let current = combine_str(&prev, current)?;
            let chunk = combine_str(current, next)?;

            chunks.push(chunk);
        }

        println!(
            "Chunked {} chunks, avg chunk size: {}",
            chunks.len(),
            if chunks.len() == 0 {
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
}

impl Chunker for SlidingWindowDelimited {
    fn chunk<'a>(&self, input: &'a str) -> Result<Vec<Chunk<'a>>, ChunkerError> {
        let Self {
            size,
            overlap,
            delimiter,
        } = self;

        let mut chunks = vec![];

        let mut total_offset = 0;
        let mut current_len = 0;
        let mut current_start = 0;

        loop {
            let Some(mut current_end) = input[total_offset..].find(*delimiter) else {
                break;
            };

            // Advance until a delimiter end is found
            let mut skipped = 0;
            while input
                .bytes()
                .nth(current_end + 1)
                .is_some_and(|ch| ch == *delimiter as u8)
            {
                current_end += 1;
                skipped += 1;
            }

            current_end += total_offset + 1;

            if current_end > input.len() {
                break;
            }

            if skipped > 0 {
                total_offset += skipped + 1;
                continue;
            }

            let sentence = &input[total_offset..current_end];

            // If the chunk fits, extend buffer and advance index
            if sentence.len() + current_len < *size {
                current_len += sentence.len();
                total_offset += sentence.len();
                continue;
            }

            // End of chunk

            let mut prev_sentences = 0;
            let mut previous = 0;
            let mut prev_start = current_start;
            while let Some(mut prev_idx) = input[..prev_start].rfind(*delimiter) {
                // Skip sequences of trailing punctuation
                let mut amt_skipped = 0;
                while input[..prev_start]
                    .bytes()
                    .nth(prev_idx - 1)
                    .is_some_and(|ch| ch == *delimiter as u8)
                {
                    amt_skipped += 1;
                    prev_idx -= 1;
                }

                // Skip the first delim since it belongs to the previous sentence.
                if prev_idx == current_start - amt_skipped - 1 {
                    prev_start -= 1;
                    continue;
                }

                if amt_skipped > 0 {
                    prev_start -= amt_skipped + 1;
                    continue;
                }

                prev_start = prev_idx - 1;
                prev_sentences += 1;
                previous = prev_idx - amt_skipped + 1;

                if prev_sentences >= *overlap {
                    break;
                }
            }

            let mut next_sentences = 0;
            let mut next = 0;
            let mut nex = input[current_end..].split_inclusive(*delimiter).peekable();

            while let Some(el) = nex.next() {
                if el.is_empty() {
                    continue;
                }

                if let Some(n) = nex.peek() {
                    if n.len() == 1 && n.contains(*delimiter) {
                        continue;
                    }
                }

                next_sentences += 1;
                next += el.len();

                if next_sentences >= *overlap {
                    break;
                }
            }

            let chunk = &input[previous..current_end + next];

            trace!("Chunked: {chunk:?}\n");

            chunks.push(Chunk::new(chunk.trim()));

            current_start = total_offset + next;
            total_offset += current_end + next - total_offset;
            current_len = 0;

            trace!("Current: {total_offset}, Start: {current_start}, End: {current_end}");
        }

        debug!(
            "Chunked {} chunks, avg chunk size: {}",
            chunks.len(),
            chunks.iter().fold(0, |acc, el| acc + el.content.len()) / chunks.len()
        );

        Ok(chunks)
    }
}

impl Default for SlidingWindowDelimited {
    fn default() -> Self {
        Self {
            size: DEFAULT_SIZE,
            overlap: 5,
            delimiter: '.',
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn pointer_sanity() {
        let input = "Hello\nWorld";
        let split = input.split_inclusive("\n").collect::<Vec<_>>();

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
        let mut chunker = SlidingWindowDelimited::default();
        chunker.size = 50;
        chunker.overlap = 2;

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
        let mut chunker = SlidingWindowDelimited::default();
        chunker.size = 10;
        chunker.overlap = 2;

        let chunks = chunker.chunk(input.trim()).unwrap();
        for chunk in chunks {
            dbg!(chunk);
        }
    }

    #[test]
    fn sliding_window_delim_skip() {
        let input = r#"Skip this........ Please do so. It would... Be a shame if you didn't."#;

        let mut chunker = SlidingWindowDelimited::default();
        chunker.size = 10;
        chunker.overlap = 2;

        let chunks = chunker.chunk(input.trim()).unwrap();
        for chunk in chunks {
            dbg!(chunk);
        }
    }

    #[test]
    fn swd_works_with_file() {
        let input = std::fs::read_to_string("content/README.md").unwrap();

        let mut chunker = Recursive::default();
        chunker.delims = &[
            "######", "#####", "####", "###", "##", "#", "```", "\n---\n", "\n___\n", "\n\n", "\n",
            " ", "",
        ];

        let chunks = chunker.chunk(input.trim()).unwrap();
        for chunk in chunks {
            dbg!(chunk);
        }
    }
}
