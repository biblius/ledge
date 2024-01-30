use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::mpsc::{Receiver, Sender},
    time::Duration,
};

use futures::Future;
use notify::{
    event::{AccessKind, CreateKind, ModifyKind, RemoveKind},
    Event, EventKind, RecommendedWatcher, Watcher,
};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use tracing_subscriber::registry::Data;

use crate::{
    db::Database,
    document::{process_directory, Document},
    error::KnawledgeError,
};

#[derive(Debug)]
pub struct NotifyHandler {
    db: Database,
    roots: HashSet<String>,
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
    pub fn new(db: Database, roots: HashSet<String>, rx: Receiver<NotifierMessage>) -> Self {
        Self { db, roots, rx }
    }

    pub fn run(self) -> Result<JoinHandle<()>, KnawledgeError> {
        let config = notify::Config::default().with_poll_interval(Duration::from_secs(1));

        let (tx, rx) = std::sync::mpsc::channel();

        let mut watcher = RecommendedWatcher::new(tx, config)?;

        for dir in self.roots.iter() {
            info!("Adding {dir} to watcher");
            watcher.watch(std::path::Path::new(dir), notify::RecursiveMode::Recursive)?;
        }

        let handle = tokio::spawn(async move {
            // We have to move the watcher in here otherwise it will drop
            let _watcher = &mut watcher;

            info!("Notifier runtime spawned");

            loop {
                let event = rx.recv().unwrap();
                match event {
                    Ok(event) => match event.kind {
                        EventKind::Create(CreateKind::Folder) => {
                            debug!("Directory created");
                            if event.paths.is_empty() {
                                continue;
                            }
                        }
                        EventKind::Create(CreateKind::File) => {
                            let path = event.paths[0].display().to_string();
                            let Some((dir, _)) = path.rsplit_once('/') else {
                                continue;
                            };

                            let Ok(Some(dir)) = self.db.get_dir_by_path(dir).await else {
                                continue;
                            };

                            let Ok(doc) = Document::read_md_file(dir.id, &path) else {
                                continue;
                            };

                            info!("Syncing file {path} with database");
                            self.db.insert_document(doc).await.unwrap();
                        }
                        EventKind::Remove(RemoveKind::File) => {
                            let path = event.paths[0].display().to_string();
                            let Some((dir, file)) = path.rsplit_once('/') else {
                                continue;
                            };

                            info!("Removing {path} from database");
                            self.db.remove_file(dir, file).await.unwrap();
                        }
                        EventKind::Remove(RemoveKind::Folder) => {
                            if !event.paths.is_empty() {
                                let path = &event.paths[0];
                                debug!("Directory removed: {}", path.display());
                                self.db.nuke_dir(&path.display().to_string()).await.unwrap();
                            }
                        }

                        // Usually comes after saving a file.
                        EventKind::Access(AccessKind::Close(e)) => {
                            dbg!(e);
                        }
                        EventKind::Modify(ModifyKind::Name(ev)) => match ev {
                            notify::event::RenameMode::To => {
                                if event.paths.is_empty() {
                                    continue;
                                }

                                let path = &event.paths[0];

                                if path.is_dir() {
                                    let path = path.display().to_string();
                                    let mut path = path.as_str();
                                    while let Some((parent, child)) = path.rsplit_once('/') {
                                        if !self.roots.contains(parent) {
                                            path = parent;
                                            continue;
                                        }

                                        let Some(root) =
                                            self.db.get_root_by_path(parent).await.unwrap()
                                        else {
                                            continue;
                                        };

                                        process_directory(
                                            &self.db,
                                            &format!("{parent}/{child}"),
                                            Some(root.id),
                                        )
                                        .await
                                        .unwrap();

                                        break;
                                    }
                                }

                                info!("File moved to {}", event.paths[0].display());
                            }

                            // Handles removal and addition of files.
                            notify::event::RenameMode::From => {
                                if event.paths.is_empty() {
                                    continue;
                                }

                                let path = event.paths[0].display().to_string();

                                info!("Moved file from {path}");

                                // Proceed only if the dir is root

                                match fs::read(&event.paths[0]) {
                                    Ok(_) => {
                                        // In case of roots, rescan whole directory
                                        if self.roots.contains(&path) {
                                            info!("Adding root {path} to database");
                                            process_directory(&self.db, path, None).await.unwrap();
                                        // In case of children, find the corresponding root
                                        // and process directories from there to preserve IDs
                                        } else if event.paths[0].is_dir() {
                                            let mut path = path.as_str();
                                            while let Some((parent, child)) = path.rsplit_once('/')
                                            {
                                                if !self.roots.contains(parent) {
                                                    path = parent;
                                                    continue;
                                                }

                                                let Some(root) =
                                                    self.db.get_root_by_path(parent).await.unwrap()
                                                else {
                                                    continue;
                                                };

                                                process_directory(
                                                    &self.db,
                                                    &format!("{parent}/{child}"),
                                                    Some(root.id),
                                                )
                                                .await
                                                .unwrap();

                                                break;
                                            }
                                        // Otherwise insert the file
                                        } else {
                                            Self::process_file(&self.db, &path).await;
                                        }
                                    }
                                    Err(_) => {
                                        let Some((dir, file)) = path.rsplit_once('/') else {
                                            continue;
                                        };

                                        self.db.nuke_dir(&path).await.unwrap();

                                        info!("Removing file/dir {path} from database");

                                        self.db.remove_file(dir, file).await.unwrap();
                                    }
                                }
                            }
                            notify::event::RenameMode::Both => {
                                let from = &event.paths[0];
                                let to = &event.paths[1];

                                info!("{} moved to {}", from.display(), to.display());
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
    }

    async fn process_file(db: &Database, path: impl AsRef<str> + Send + Sync) {
        let path = path.as_ref();

        let Some((dir, _)) = path.rsplit_once('/') else {
            return;
        };

        let Ok(Some(dir)) = db.get_dir_by_path(dir).await else {
            return;
        };

        let Ok(doc) = Document::read_md_file(dir.id, path) else {
            return;
        };

        info!("Syncing file {path} with database");
        db.insert_document(doc).await.unwrap();
    }
}
