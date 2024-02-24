use std::{
    collections::HashSet,
    fs,
    sync::{
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
    time::Duration,
};

use notify::{
    event::{CreateKind, DataChange, ModifyKind, RemoveKind, RenameMode},
    EventKind, RecommendedWatcher, Watcher,
};
use tokio::task::JoinHandle;
use tracing::{error, info};

use crate::{
    db::Database,
    document::{process_directory, Document, DocumentMeta},
    error::KnawledgeError,
};

#[derive(Debug)]
pub struct NotifyHandler {
    db: Database,
    roots: HashSet<String>,
    _rx: Receiver<NotifierMessage>,
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
        Self { db, roots, _rx: rx }
    }

    // TODO: Shutdown channel
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

            let rx = Arc::new(Mutex::new(rx));
            loop {
                let rx = rx.clone();
                let event = tokio::task::spawn_blocking(move || rx.lock().unwrap().recv()).await;

                if event.is_err() {
                    continue;
                }

                let event = event.unwrap().unwrap();

                if let Err(e) = event {
                    error!("Error reading inotify event: {e}");
                    continue;
                }

                let event = event.unwrap();

                match event.kind {
                    EventKind::Create(CreateKind::Folder) => {
                        if event.paths.is_empty() {
                            continue;
                        }
                        let path = event.paths[0].display().to_string();
                        let Some((parent, name)) = path.rsplit_once('/') else {
                            continue;
                        };
                        let parent = self.db.get_dir_by_path(parent).await.unwrap();

                        // Parent IDs always must exist here since we know the
                        // dir is a child in a watched directory
                        if let Some(parent) = parent {
                            info!("Syncing directory {path} with database");
                            self.db
                                .insert_dir(&path, name, Some(parent.id))
                                .await
                                .unwrap();
                        }
                    }
                    EventKind::Create(CreateKind::File) => {
                        if event.paths.is_empty() {
                            continue;
                        }
                        let path = event.paths[0].display().to_string();
                        info!("Syncing file {path} with database");
                        Self::process_file(&self.db, path).await;
                    }
                    EventKind::Remove(RemoveKind::File) => {
                        if event.paths.is_empty() {
                            continue;
                        }
                        let path = event.paths[0].display().to_string();
                        info!("Removing file {path} from database");
                        self.db.remove_doc(&path).await.unwrap();
                    }
                    EventKind::Remove(RemoveKind::Folder) => {
                        if event.paths.is_empty() {
                            continue;
                        }
                        let path = &event.paths[0].display().to_string();
                        info!("Removing directory {path} from database");
                        self.db.remove_dir(path).await.unwrap();
                    }
                    EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
                        if event.paths.is_empty() {
                            continue;
                        }

                        let path = &event.paths[0];

                        info!("File moved to {}", path.display());

                        if path.is_dir() {
                            let path = path.display().to_string();
                            info!("Syncing directory {path} with database");
                            let mut path = path.as_str();
                            while let Some((parent, child)) = path.rsplit_once('/') {
                                if !self.roots.contains(parent) {
                                    path = parent;
                                    continue;
                                }

                                let Some(root) = self.db.get_root_by_path(parent).await.unwrap()
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
                        } else if path.is_file() {
                            let path = path.display().to_string();
                            info!("Syncing file {path} with database");
                            Self::process_file(&self.db, path).await;
                        }
                    }
                    EventKind::Modify(ModifyKind::Data(DataChange::Any)) => {
                        if event.paths.is_empty() {
                            continue;
                        }
                        let path = event.paths[0].display().to_string();
                        info!("Syncing file {path} with database");
                        let meta = DocumentMeta::read_from_file(&path).unwrap();
                        self.db.update_doc_by_path(&path, &meta).await.unwrap();
                    }
                    // Handles removal and addition of files.
                    EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
                        if event.paths.is_empty() {
                            continue;
                        }

                        let path = event.paths[0].display().to_string();

                        info!("File moved from {path}");

                        match fs::metadata(&event.paths[0]) {
                            Ok(_) => {
                                // In case of roots, rescan whole directory
                                if self.roots.contains(&path) {
                                    info!("Adding root {path} to database");
                                    process_directory(&self.db, path, None).await.unwrap();
                                // In case of children, find the corresponding root
                                // and process directories from there to preserve IDs
                                } else if event.paths[0].is_dir() {
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
                                // Otherwise insert the file
                                } else {
                                    Self::process_file(&self.db, &path).await;
                                }
                            }
                            Err(_) => {
                                info!("Removing file/dir {path} from database");

                                self.db.remove_dir(&path).await.unwrap();
                                self.db.remove_doc(&path).await.unwrap();
                            }
                        }
                    }
                    e => info!("Inotify event: {e:?}"),
                }
            }
        });

        Ok(handle)
    }

    async fn process_file(db: &Database, path: impl AsRef<str> + Send + Sync) {
        let path = path.as_ref();

        let Some((dir, name)) = path.rsplit_once('/') else {
            return;
        };

        let Ok(Some(dir)) = db.get_dir_by_path(dir).await else {
            return;
        };

        // Here we already have the canonicalized path
        let Ok((doc, meta)) = Document::new(dir.id, name.to_string(), path.to_string()) else {
            return;
        };

        db.insert_doc(&doc, &meta).await.unwrap();
    }
}
