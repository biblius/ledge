use clap::Parser;

#[derive(Debug, Clone, Parser)]
pub struct Config {
    /// The title for the front end
    #[arg(short, long)]
    pub title: Option<String>,

    /// The list of directories to initially include in knawledger
    #[arg(short, long)]
    pub directories: Vec<String>,
}
