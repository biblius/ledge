use std::{
    path::Path,
    sync::mpsc::{Receiver, Sender},
    time::Duration,
};

use notify::{
    event::{AccessKind, CreateKind, ModifyKind, RemoveKind},
    EventKind, RecommendedWatcher, Watcher,
};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::{db::Database, error::KnawledgeError};

#[derive(Debug)]
pub struct NotifyHandler {
    db: Database,
    directories: Vec<String>,
    rx: Receiver<NotifierMessage>,
}

#[derive(Debug)]
pub struct NotifierHandle {
    pub tx: Sender<NotifierMessage>,
    pub handle: JoinHandle<()>,
}

#[derive(Debug)]
pub enum NotifierMessage {
    AddWatch(String),
    Terminate,
}

impl NotifyHandler {
    pub fn new(db: Database, directories: Vec<String>, rx: Receiver<NotifierMessage>) -> Self {
        Self {
            db,
            directories,
            rx,
        }
    }

    pub fn run(self) -> Result<JoinHandle<()>, KnawledgeError> {
        let db = self.db.clone();

        let config = notify::Config::default().with_poll_interval(Duration::from_secs(1));

        let (tx, rx) = std::sync::mpsc::channel();

        let mut watcher = RecommendedWatcher::new(tx, config)?;

        for dir in self.directories.iter() {
            info!("Adding {dir} to watcher");
            watcher.watch(std::path::Path::new(dir), notify::RecursiveMode::Recursive)?;
        }

        let handle = tokio::spawn(async move {
            // We have to move the watcher in here otherwise it will drop
            let watcher = &mut watcher;

            info!("Notifier runtime spawned");

            'main: loop {
                let event = rx.recv().unwrap();
                match event {
                    Ok(event) => match event.kind {
                        EventKind::Create(CreateKind::Folder) => {
                            debug!("Directory created");
                            if event.paths.is_empty() {
                                continue;
                            }

                            let path = &event.paths[0];
                            match watcher
                                .watch(Path::new(path), notify::RecursiveMode::NonRecursive)
                            {
                                Ok(_) => info!("Watching {}", path.display()),
                                Err(e) => error!("Watch failed: {e}"),
                            }
                        }
                        EventKind::Remove(RemoveKind::File) => {
                            if !event.paths.is_empty() {
                                let path = &event.paths[0];
                                debug!("File removed: {}", path.display());
                            }
                        }
                        EventKind::Remove(RemoveKind::Folder) => {
                            if !event.paths.is_empty() {
                                let path = &event.paths[0];
                                debug!("Directory removed: {}", path.display());
                            }
                        }
                        EventKind::Access(AccessKind::Close(e)) => {
                            dbg!(e);
                        }
                        EventKind::Modify(ModifyKind::Name(ev)) => match ev {
                            notify::event::RenameMode::To => {
                                info!("Moved file to {}", event.paths[0].display());
                            }
                            notify::event::RenameMode::From => {
                                info!("Moved file from {}", event.paths[0].display());
                            }
                            notify::event::RenameMode::Both => {
                                let from = &event.paths[0];
                                let to = &event.paths[1];

                                dbg!(from, to);

                                for segment in from.iter().rev() {
                                    let Some(segment) = segment.to_str() else {
                                        continue;
                                    };

                                    dbg!(segment);

                                    // Attempt to find the root directory of the change and update the db
                                    // entry accodringly
                                    if let Some(root) =
                                        self.directories.iter().find(|seg| seg.as_str() == segment)
                                    {
                                        // db.update_root(segment, new);
                                        info!("Watched directory changed, resyncing");
                                        break;
                                    }
                                }

                                info!("{} moved to {}", from.display(), to.display());

                                if to.is_dir() {}
                            }
                            e => warn!("Unhandled event: {e:?}"),
                        },
                        e => warn!("Unhandled event: {e:?}"),
                    },
                    Err(e) => {
                        error!("Error reading inotify event: {e}")
                    }
                }
            }
        });

        Ok(handle)
        // tokio::spawn(future)
    }
}
