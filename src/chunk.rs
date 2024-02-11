use std::ptr;

pub trait Chunker<'a> {
    type OutChunk;

    fn chunk(&self, input: &'a str) -> Vec<Self::OutChunk>;
}

#[derive(Debug)]
pub enum ChunkerError {
    Config(String),
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

impl<'a> Chunker<'a> for SlidingWindow {
    type OutChunk = Chunk<'a>;

    fn chunk(&self, input: &'a str) -> Vec<Chunk<'a>> {
        let SlidingWindow { size, overlap } = self;

        let input = input.trim();

        if input.is_empty() {
            return vec![];
        }

        if input.len() <= size + overlap {
            return vec![Chunk::new(input)];
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
            chunks.push(chunk);

            start = end;
            end += size;
        }

        chunks
    }
}

const DEFAULT_DELIMS: &[&str] = &["\n\n", "\n", " ", ""];

#[derive(Debug)]
pub struct Recursive<'a> {
    /// Target chunk size.
    size: usize,

    /// The delimiters to use when splitting.
    delims: &'a [&'a str],
}

impl<'a> Recursive<'a> {
    pub fn new(size: usize) -> Self {
        Recursive {
            size,
            delims: DEFAULT_DELIMS,
        }
    }

    pub fn new_delimiters(size: usize, delimiters: &'a [&'a str]) -> Self {
        Recursive {
            size,
            delims: delimiters,
        }
    }

    /// Chunk the input using this instance's delimiters.
    ///
    /// `input` - The input text to chunk
    ///
    /// `idx` - The current delimiter index.
    ///
    /// `buffer` - A string in which the current split contents are stored if they are smaller than
    /// this instance's `size'.
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
    fn chunk_recursive(
        &self,
        input: &str,
        idx: usize,
        buffer: &mut String,
        chunks: &mut Vec<String>,
    ) {
        let Recursive { size, delims } = self;

        if idx >= delims.len() {
            return;
        }

        let split = input.split_inclusive(delims[idx]);

        for chunk in split {
            if chunk.len() + buffer.len() <= *size {
                // Store the current chunk in the buffer
                // so we can merge it with later ones
                buffer.push_str(chunk);
                continue;
            }

            // Can't store current chunk with existing buf
            // If the buf is not empty, add it to the chunks and reset buffer
            if !buffer.is_empty() {
                chunks.push(buffer.clone());
                buffer.clear();

                // Check again and reset loop if it fits
                if chunk.len() <= *size {
                    buffer.push_str(chunk);
                    continue;
                }
            }

            // If the chunk is still too large, execute another round of splitting
            self.chunk_recursive(chunk, idx + 1, buffer, chunks);
        }

        if !buffer.is_empty() {
            chunks.push(buffer.clone());
            buffer.clear();
        }
    }
}

#[derive(Debug)]
pub struct ChunkOwned {
    content: String,
}

impl ChunkOwned {
    pub fn new(content: String) -> Self {
        Self { content }
    }
}

impl<'input, 'r> Chunker<'input> for Recursive<'r> {
    type OutChunk = ChunkOwned;

    fn chunk(&self, input: &'input str) -> Vec<Self::OutChunk> {
        let mut buffer = String::with_capacity(self.size);
        let mut chunks = vec![];
        self.chunk_recursive(input, 0, &mut buffer, &mut chunks);
        chunks
            .into_iter()
            .filter_map(|chunk| (!chunk.trim().is_empty()).then_some(ChunkOwned::new(chunk)))
            .collect()
    }
}

#[test]
fn sliding_window_works() {
    let input = "Sticks and stones may break my bones, but words will never leverage agile frameworks to provide a robust synopsis for high level overviews.";
    let window = SlidingWindow::new(30, 20).unwrap();
    let chunks = window.chunk(input);

    assert_eq!(&input[0..50], chunks[0].content);
    assert_eq!(&input[10..80], chunks[1].content);
    assert_eq!(&input[40..110], chunks[2].content);
    assert_eq!(&input[70..input.len()], chunks[3].content);
}

#[test]
fn sliding_window_empty() {
    let input = "";
    let window = SlidingWindow::new(1, 0).unwrap();
    let chunks = window.chunk(input);

    assert!(chunks.is_empty());
}

#[test]
fn sliding_window_small_input() {
    let input = "Foobar";
    let window = SlidingWindow::new(30, 20).unwrap();
    let chunks = window.chunk(input);

    assert_eq!(input, chunks[0].content);
}

#[test]
fn pointer_sanity() {
    let input = "Hello\nWorld";
    let split = input.split_inclusive("\n").collect::<Vec<_>>();

    let one = split[0];
    let two = split[1];

    let combined = ptr::slice_from_raw_parts(one.as_ptr(), one.len() + two.len());
    unsafe {
        assert_eq!(input, std::str::from_utf8(&*combined).unwrap());
    }
}

#[test]
fn recursive_works() {
    let input = r#"
What I Worked On

February 2021

Before college the two main things I worked on, outside of school, were writing and programming. I didn't write essays. I wrote what beginning writers were supposed to write then, and probably still are: short stories. My stories were awful. They had hardly any plot, just characters with strong feelings, which I imagined made them deep.

The first programs I tried writing were on the IBM 1401 that our school district used for what was then called "data processing." This was in 9th grade, so I was 13 or 14. The school district's 1401 happened to be in the basement of our junior high school, and my friend Rich Draves and I got permission to use it. It was like a mini Bond villain's lair down there, with all these alien-looking machines — CPU, disk drives, printer, card reader — sitting up on a raised floor under bright fluorescent lights.
"#;
    let chunker = Recursive::new(100);
    let chunks = chunker.chunk(input.trim());

    for chunk in chunks {
        assert!(chunk.content.len() <= 100);
    }
}

#[test]
fn recursive_small_input_custom_delims() {
    let input = "Supercalifragilisticexpialadocius";
    let chunker = Recursive::new_delimiters(5, &["foo"]);
    let chunks = chunker.chunk(input);
    assert!(chunks.is_empty());
}
