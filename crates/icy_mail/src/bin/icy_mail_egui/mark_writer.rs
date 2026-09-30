use std::{
    sync::mpsc,
    thread::{self, JoinHandle},
};

use eframe::egui;
use icy_mail::state::ReadStateSnapshot;

/// What a failed save was about, so the warning names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkKind {
    Read,
    Star,
}

enum Job {
    Save(ReadStateSnapshot, MarkKind, egui::Context),
    Flush(mpsc::Sender<()>),
}

/// Saves read marks and stars on a background thread, so a slow disk never stalls the UI.
///
/// Queued saves of the same packet collapse into the newest one; dropping the writer finishes them.
pub struct MarkWriter {
    jobs: Option<mpsc::Sender<Job>>,
    errors: mpsc::Receiver<(MarkKind, String)>,
    thread: Option<JoinHandle<()>>,
}

impl MarkWriter {
    pub fn new() -> Self {
        let (jobs, receiver) = mpsc::channel::<Job>();
        let (report, errors) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("icy_mail marks".into())
            .spawn(move || {
                while let Ok(job) = receiver.recv() {
                    let mut saves: Vec<(ReadStateSnapshot, MarkKind, egui::Context)> = Vec::new();
                    let mut flushes = Vec::new();
                    for job in std::iter::once(job).chain(receiver.try_iter()) {
                        match job {
                            Job::Save(snapshot, kind, context) => {
                                saves.retain(|(queued, _, _)| queued.path() != snapshot.path());
                                saves.push((snapshot, kind, context));
                            }
                            Job::Flush(done) => flushes.push(done),
                        }
                    }
                    for (snapshot, kind, context) in saves {
                        if let Err(error) = snapshot.write() {
                            let _ = report.send((kind, error.to_string()));
                            context.request_repaint();
                        }
                    }
                    for done in flushes {
                        let _ = done.send(());
                    }
                }
            })
            .ok();
        Self {
            jobs: thread.as_ref().map(|_| jobs),
            errors,
            thread,
        }
    }

    pub fn save(&self, snapshot: ReadStateSnapshot, kind: MarkKind, context: &egui::Context) -> Result<(), String> {
        match &self.jobs {
            Some(jobs) => jobs.send(Job::Save(snapshot, kind, context.clone())).map_err(|error| error.to_string()),
            None => snapshot.write().map_err(|error| error.to_string()),
        }
    }

    /// Waits until every queued save is on disk.
    pub fn flush(&self) {
        let Some(jobs) = &self.jobs else {
            return;
        };
        let (done, wait) = mpsc::channel();
        if jobs.send(Job::Flush(done)).is_ok() {
            let _ = wait.recv();
        }
    }

    /// Failed saves since the last call.
    pub fn errors(&self) -> impl Iterator<Item = (MarkKind, String)> + '_ {
        self.errors.try_iter()
    }
}

impl Drop for MarkWriter {
    fn drop(&mut self) {
        self.jobs = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
