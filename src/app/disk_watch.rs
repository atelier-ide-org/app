//! What changed on disk since the interface last read it: the open files another program
//! rewrote, the listed folders that gained or lost entries. The dates are asked in the
//! background: on a slow disk (a WSL distribution, a network share) each one takes a moment,
//! and the window froze every second while it asked them itself.

use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
    time::SystemTime,
};

/// What the interface knows of the disk.
#[derive(Default)]
pub struct Ask {
    /// The open files, and the modification time each was read at.
    pub files: Vec<(PathBuf, Option<SystemTime>)>,
    /// The listed folders, and the modification time each was listed at (None: not known).
    pub folders: Vec<(PathBuf, Option<Option<SystemTime>>)>,
}

/// What is no longer as the interface knows it.
#[derive(Debug, Default, PartialEq)]
pub struct Changes {
    pub files: Vec<PathBuf>,
    pub folders: Vec<PathBuf>,
}

/// Reads the dates. A file that is gone has not changed: its document stays as it is.
pub fn changes(ask: Ask) -> Changes {
    let modified = crate::project::modified;
    Changes {
        files: (ask.files.into_iter())
            .filter(|(path, seen)| modified(path).is_some_and(|m| *seen != Some(m)))
            .map(|(path, _)| path)
            .collect(),
        folders: (ask.folders.into_iter())
            .filter(|(path, seen)| *seen != Some(modified(path)))
            .map(|(path, _)| path)
            .collect(),
    }
}

/// One question at a time, answered by a thread.
#[derive(Default)]
pub struct Watch {
    pending: Option<Receiver<Changes>>,
}

impl Watch {
    /// A question is still being answered.
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }

    /// Asks in the background; `wake` is called when something changed.
    pub fn ask(&mut self, ask: Ask, wake: impl Fn() + Send + 'static) {
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        std::thread::spawn(move || {
            let changes = changes(ask);
            let any = changes != Changes::default();
            if tx.send(changes).is_ok() && any {
                wake();
            }
        });
    }

    /// The answer, once.
    pub fn take(&mut self) -> Option<Changes> {
        match self.pending.as_ref()?.try_recv() {
            Ok(changes) => {
                self.pending = None;
                Some(changes)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.pending = None;
                None
            }
        }
    }

    /// Waits for the answer.
    #[cfg(test)]
    pub fn wait(&mut self) -> Option<Changes> {
        self.pending.take()?.recv().ok()
    }
}

#[cfg(test)]
#[path = "../../test/app/disk_watch/tests.rs"]
mod tests;
