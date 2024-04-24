use std::fs;

use crate::error::LedgeknawError;

use self::chunk::Chunker;

pub mod chunk;

/// Chunk all the files in the specified directory. If `out` is provided, the chunks
/// will be written to the given directory.
pub fn prepare_chunks<T: Chunker>(
    chunker: &T,
    directory: &str,
    out: Option<&str>,
) -> Result<(), LedgeknawError> {
    // TODO: Handle bad out directory

    let entries = fs::read_dir(directory)?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    for entry in entries {
        if entry.path().is_dir() {
            prepare_chunks(chunker, &entry.path().display().to_string(), out)?;
            continue;
        }

        let file = fs::read_to_string(entry.path())?;
        let chunks = chunker.chunk(&file)?;

        if let Some(ref out) = out {
            fs::write(
                format!(
                    "{}/{}.json",
                    out,
                    entry.path().file_name().unwrap().to_str().unwrap()
                ),
                serde_json::to_string_pretty(&chunks)?,
            )?;
        }
    }

    Ok(())
}
