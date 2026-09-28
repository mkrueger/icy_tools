//! Autosave of unsaved documents and recovery after a crash, see [`icy_draw::recovery`].

use super::*;
use icy_draw::recovery::{self as store, Fingerprint, Orphan, Recovery, RecoveryKind, Snapshot};
use std::time::{Duration, Instant};

/// How often unsaved changes are written at most.
pub(super) const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(2);
/// Snapshots of very large documents are taken less often, so they use at most this share of time.
const MAX_AUTOSAVE_LOAD: u32 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DiskState {
    Untitled,
    Unchanged,
    Changed,
    Missing,
}

/// A recovery file offered in the recovery dialog.
pub(super) struct Offer {
    pub orphan: Orphan,
    pub disk: DiskState,
    confirm_discard: bool,
}

impl Offer {
    fn new(orphan: Orphan) -> Self {
        let disk = match &orphan.header {
            Ok(header) => match &header.path {
                None => DiskState::Untitled,
                Some(path) => match std::fs::read(path) {
                    Ok(bytes) if header.disk == Some(Fingerprint::of(&bytes)) => DiskState::Unchanged,
                    Ok(_) => DiskState::Changed,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => DiskState::Missing,
                    Err(_) => DiskState::Changed,
                },
            },
            Err(_) => DiskState::Untitled,
        };
        Self {
            orphan,
            disk,
            confirm_discard: false,
        }
    }
}

impl DrawApp {
    /// Starts autosaving into `dir`. Without it, unsaved changes are lost in a crash, which is
    /// reported to the user.
    pub fn enable_recovery(&mut self, dir: Option<PathBuf>) {
        let result = dir
            .ok_or_else(|| fl!("recovery-no-directory"))
            .and_then(|dir| Recovery::start(&dir).map_err(|error| error.to_string()));
        match result {
            Ok(recovery) => self.recovery = Some(recovery),
            Err(error) => {
                log::error!("autosave unavailable: {error}");
                self.dialog = Some(Dialog::Error(fl!("recovery-unavailable", error = error)));
            }
        }
    }

    /// Offers the documents of editors that ended without saving.
    pub fn offer_recovery(&mut self) {
        if let Some(recovery) = &self.recovery {
            self.offers = recovery.orphans().into_iter().map(Offer::new).collect();
        }
    }

    /// Restores the recovery file `id` in this window (used for windows opened by the dialog).
    pub fn restore_recovered(&mut self, id: &str) {
        let Some(dir) = self.recovery.as_ref().map(|recovery| recovery.dir().to_path_buf()) else {
            return;
        };
        match store::claim(&dir, id) {
            Ok(orphan) => {
                if let Err(error) = self.restore_here(orphan) {
                    self.show_start = true;
                    self.dialog = Some(Dialog::Error(fl!("recovery-restore-failed", error = error)));
                }
            }
            Err(error) => {
                self.show_start = true;
                self.dialog = Some(Dialog::Error(error));
            }
        }
    }

    fn recovery_snapshot(&self) -> Result<Snapshot, String> {
        if let Some(editor) = self.font_editor.as_ref().filter(|editor| !editor.apply_target) {
            return editor.recovery_snapshot();
        }
        if let Some(editor) = &self.animation {
            return Ok(editor.recovery_snapshot());
        }
        if let Some(font) = &self.charfont {
            return font.recovery_snapshot(&self.document);
        }
        self.document.recovery_snapshot()
    }

    fn restore_snapshot(&mut self, snapshot: &Snapshot) -> Result<(), String> {
        match snapshot.kind {
            RecoveryKind::Ansi => self.replace(Document::from_recovery(snapshot)?),
            RecoveryKind::CharFont => {
                let font = icy_draw::charfont::CharFontDocument::from_recovery(snapshot)?;
                self.replace(font.document());
                self.charfont = Some(font);
            }
            RecoveryKind::BitFont => {
                let editor = super::super::font::FontEditor::from_recovery(snapshot)?;
                self.replace(Document::new(Size::new(80, 25)));
                self.font_editor = Some(editor);
            }
            RecoveryKind::Animation => {
                let editor = super::super::animation::AnimationEditor::from_recovery(snapshot)?;
                self.replace(Document::new(Size::new(80, 25)));
                self.animation = Some(editor);
            }
        }
        Ok(())
    }

    /// Whether restoring may replace the document of this window.
    fn recovery_window_free(&self) -> bool {
        !self.collab.active && (self.show_start || !self.modified())
    }

    /// Opens the recovered document here and moves its recovery file into this editor's entry.
    fn restore_here(&mut self, orphan: Orphan) -> Result<(), String> {
        let snapshot = orphan.load()?;
        self.restore_snapshot(&snapshot)?;
        // The old file is only deleted once the content is stored in this editor's entry.
        let recovery = self.recovery.as_mut().ok_or("autosave is unavailable")?;
        recovery.store(&snapshot).map_err(|error| error.to_string())?;
        recovery.flush();
        if let Some(Err(error)) = recovery.take_status() {
            return Err(error);
        }
        self.next_autosave = None;
        if let Err(error) = orphan.discard() {
            log::warn!("could not remove the restored recovery file: {error}");
        }
        Ok(())
    }

    fn restore_in_new_window(&mut self, offer: Offer) -> Result<(), String> {
        let id = offer.orphan.id().to_owned();
        // The new window claims the file once this one releases it.
        drop(offer);
        let result = std::env::current_exe().and_then(|executable| std::process::Command::new(executable).arg("--recover").arg(&id).spawn());
        if let Err(error) = result {
            if let Some(dir) = self.recovery.as_ref().map(|recovery| recovery.dir().to_path_buf()) {
                if let Ok(orphan) = store::claim(&dir, &id) {
                    self.offers.push(Offer::new(orphan));
                }
            }
            return Err(error.to_string());
        }
        Ok(())
    }

    pub(super) fn restore_offer(&mut self, index: usize) {
        let offer = self.offers.remove(index);
        let result = if self.recovery_window_free() {
            let id = offer.orphan.id().to_owned();
            match self.restore_here(offer.orphan) {
                Ok(()) => Ok(()),
                Err(error) => {
                    // Keep offering it; the file is still on disk.
                    if let Some(dir) = self.recovery.as_ref().map(|recovery| recovery.dir().to_path_buf()) {
                        if let Ok(orphan) = store::claim(&dir, &id) {
                            self.offers.insert(index.min(self.offers.len()), Offer::new(orphan));
                        }
                    }
                    Err(error)
                }
            }
        } else {
            self.restore_in_new_window(offer)
        };
        if let Err(error) = result {
            self.dialog = Some(Dialog::Error(fl!("recovery-restore-failed", error = error)));
        }
    }

    /// Writes the unsaved document at most every [`AUTOSAVE_INTERVAL`] and removes the snapshot
    /// once nothing is unsaved.
    pub(super) fn autosave(&mut self, context: &egui::Context) {
        if self.recovery.is_none() {
            return;
        }
        let modified = !self.allow_close && self.modified();
        let now = Instant::now();
        let due = self.next_autosave.is_none_or(|time| time <= now);
        let snapshot = (modified && due).then(|| {
            let started = Instant::now();
            (self.recovery_snapshot(), started.elapsed())
        });
        let Some(recovery) = self.recovery.as_mut() else {
            return;
        };
        match recovery.take_status() {
            Some(Err(error)) if !self.autosave_failing => {
                self.autosave_failing = true;
                self.autosave_error = Some(error);
            }
            Some(Ok(())) => self.autosave_failing = false,
            _ => {}
        }
        if !modified {
            recovery.clear();
            self.next_autosave = None;
            return;
        }
        if let Some((snapshot, cost)) = snapshot {
            let stored = snapshot.and_then(|snapshot| recovery.store(&snapshot).map_err(|error| error.to_string()));
            if let Err(error) = stored {
                log::error!("autosave snapshot failed: {error}");
                if !self.autosave_failing {
                    self.autosave_failing = true;
                    self.autosave_error = Some(error);
                }
            }
            self.next_autosave = Some(now + AUTOSAVE_INTERVAL.max(cost * MAX_AUTOSAVE_LOAD));
        }
        // Keeps checking while idle, since edits right before going idle must be written too.
        if let Some(time) = self.next_autosave {
            context.request_repaint_after(time.saturating_duration_since(now));
        }
    }

    /// Removes the snapshot synchronously when the window closes after the user decided about
    /// the unsaved changes.
    pub(super) fn finish_recovery(&mut self) {
        if let Some(recovery) = &mut self.recovery {
            recovery.clear();
            recovery.flush();
        }
    }

    pub(super) fn recovery_dialog(&mut self, context: &egui::Context) -> bool {
        #[derive(Clone, Copy)]
        enum Action {
            Later,
            RestoreAll,
        }
        let mut restore = None;
        let mut discard = None;
        let free = self.recovery_window_free();
        let restorable = self.offers.iter().filter(|offer| offer.orphan.header.is_ok()).count();
        let response = appearance::Dialog::new("recovery")
            .title(fl!("recovery-title"))
            .subtitle(fl!("recovery-description"))
            .size(DialogSize::Large)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    for (index, offer) in self.offers.iter_mut().enumerate() {
                        let id = offer.orphan.id().to_owned();
                        ui.push_id(id, |ui| {
                            offer_row(ui, offer, index, free, &mut restore, &mut discard);
                        });
                    }
                });
                let mut buttons = vec![DialogButton::cancel(fl!("recovery-later"), Action::Later)];
                if restorable > 1 {
                    buttons.push(DialogButton::primary(fl!("recovery-restore-all"), Action::RestoreAll));
                }
                dialog.buttons(buttons);
            });
        if let Some(index) = discard {
            let offer = self.offers.remove(index);
            if let Err(error) = offer.orphan.discard() {
                self.dialog = Some(Dialog::Error(error.to_string()));
            }
        }
        if let Some(index) = restore {
            self.restore_offer(index);
        }
        match response.action {
            Some(Action::Later) => {
                self.offers.clear();
                false
            }
            Some(Action::RestoreAll) => {
                let mut index = 0;
                while index < self.offers.len() {
                    if self.offers[index].orphan.header.is_ok() {
                        let before = self.offers.len();
                        self.restore_offer(index);
                        if self.offers.len() == before {
                            // Restoring failed and the offer was kept.
                            index += 1;
                        }
                    } else {
                        index += 1;
                    }
                }
                !self.offers.is_empty()
            }
            None if response.dismissed => {
                self.offers.clear();
                false
            }
            None => !self.offers.is_empty(),
        }
    }
}

fn offer_row(ui: &mut egui::Ui, offer: &mut Offer, index: usize, free: bool, restore: &mut Option<usize>, discard: &mut Option<usize>) {
    egui::Frame::group(ui.style()).inner_margin(10).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let buttons = 230.0;
            ui.vertical(|ui| {
                ui.set_max_width((ui.available_width() - buttons).max(120.0));
                let (title, details) = match &offer.orphan.header {
                    Ok(header) => {
                        let kind = match header.kind {
                            RecoveryKind::Ansi => fl!("recovery-kind-ansi"),
                            RecoveryKind::CharFont => fl!("recovery-kind-charfont"),
                            RecoveryKind::BitFont => fl!("recovery-kind-bitfont"),
                            RecoveryKind::Animation => fl!("recovery-kind-animation"),
                        };
                        let time = chrono::DateTime::from_timestamp(header.saved_at as i64, 0)
                            .map(|time| time.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_default();
                        let folder = header.path.as_ref().and_then(|path| path.parent()).map(|path| path.display().to_string());
                        let details = [Some(kind), folder, Some(fl!("recovery-saved-at", time = time))]
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>()
                            .join(" · ");
                        (header.title().unwrap_or_else(|| fl!("unsaved-title")), details)
                    }
                    Err(error) => (offer.orphan.id().to_owned(), fl!("recovery-damaged", error = error.clone())),
                };
                ui.label(appearance::bold(ui, title));
                ui.label(egui::RichText::new(details).weak().size(12.0));
                let note = match offer.disk {
                    DiskState::Changed => Some(fl!("recovery-disk-changed")),
                    DiskState::Missing => Some(fl!("recovery-disk-missing")),
                    DiskState::Untitled | DiskState::Unchanged => None,
                };
                if let Some(note) = note {
                    ui.label(egui::RichText::new(note).size(12.0).color(ui.visuals().warn_fg_color));
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if offer.confirm_discard {
                    let danger =
                        egui::Button::new(egui::RichText::new(fl!("recovery-discard-confirm")).color(Color32::WHITE)).fill(ui.visuals().error_fg_color);
                    if ui.add(danger).clicked() {
                        *discard = Some(index);
                    }
                    if ui.button(fl!("recovery-keep")).clicked() {
                        offer.confirm_discard = false;
                    }
                } else {
                    if offer.orphan.header.is_ok() {
                        let response = ui.add(appearance::primary_button(fl!("recovery-restore")));
                        let response = if free { response } else { response.on_hover_text(fl!("recovery-new-window")) };
                        if response.clicked() {
                            *restore = Some(index);
                        }
                    }
                    if ui.button(fl!("recovery-discard")).clicked() {
                        offer.confirm_discard = true;
                    }
                }
            });
        });
    });
    ui.add_space(4.0);
}
