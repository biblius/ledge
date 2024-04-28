use super::{Chunk, Chunker, ChunkerError, DEFAULT_OVERLAP, DEFAULT_SIZE};
use tracing::{debug, trace};

/// The most basic of chunkers.
///
/// `size` determines the base amount for every chunk and
/// `overlap` determines how much back and front characters
/// to extend the base with.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sliding_window_works() {
        let input = "Sticks and stones may break my bones, but words will never leverage agile frameworks to provide a robust synopsis for high level overviews.";
        let window = SlidingWindow::new(30, 20).unwrap();
        let chunks = window.chunk(input).unwrap();

        assert_eq!(&input[0..50], chunks[0].content);
        assert_eq!(&input[10..80], chunks[1].content);
        assert_eq!(&input[40..110], chunks[2].content);
        assert_eq!(&input[70..], chunks[3].content);
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
}
