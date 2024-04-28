use crate::llm::chunk::concat;

use super::{Chunk, Chunker, ChunkerError, DEFAULT_OVERLAP, DEFAULT_SIZE};

/// Default delimiters for the [recursive chunker][Recursive].
const DEFAULT_DELIMS: &[&str] = &["\n\n", "\n", " ", ""];

/// Default delimiters for the [recursive chunker][Recursive] when constructed with
/// [Recursive::markdown].
const MARKDOWN_DELIMS: &[&str] = &[
    "#", "##", "###", "####", "#####", "######", "\n```", "\n---\n", "\n___\n", "\n\n", "\n", " ",
    "",
];

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
                    let chunk = concat(current, next)?;
                    chunks.push(chunk);
                } else {
                    let chunk = concat(current, &next[..=self.overlap])?;
                    chunks.push(chunk);
                }
                continue;
            }

            // Special case for last item
            if i == splits.len() - 1 {
                let prev = &splits[i - 1];
                if prev.len() <= self.overlap {
                    let chunk = concat(prev, current)?;
                    chunks.push(chunk);
                } else {
                    let chunk = concat(&prev[self.overlap..], current)?;
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

            let current = concat(prev, current)?;
            let chunk = concat(current, next)?;

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

#[cfg(test)]
mod tests {
    use super::super::tests::INPUT;
    use super::*;

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
}
