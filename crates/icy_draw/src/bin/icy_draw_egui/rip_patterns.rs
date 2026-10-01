//! RIP bitmap patterns, displayed most-significant bit first, as stored on disk.
use eframe::egui;
use icy_draw::fl;
use icy_parser_core::RipCommand;

pub fn fill_command(rows: [u8; 8], color: u16) -> RipCommand {
    let [c1, c2, c3, c4, c5, c6, c7, c8] = rows.map(u16::from);
    RipCommand::FillPattern {
        c1,
        c2,
        c3,
        c4,
        c5,
        c6,
        c7,
        c8,
        col: color,
    }
}

pub fn fill_editor(ui: &mut egui::Ui, rows: &mut [u8; 8]) {
    ui.label(fl!("rip-pattern-fill-hint"));
    egui::Grid::new("rip-custom-fill-bits").spacing(egui::vec2(2.0, 2.0)).show(ui, |ui| {
        for row in rows.iter_mut() {
            for bit in (0..8).rev() {
                if ui
                    .add(egui::Button::new(" ").selected(*row & (1 << bit) != 0).min_size(egui::vec2(22.0, 22.0)))
                    .clicked()
                {
                    *row ^= 1 << bit;
                }
            }
            ui.end_row();
        }
    });
    ui.horizontal(|ui| {
        if ui.button(fl!("rip-pattern-clear")).clicked() {
            *rows = [0; 8];
        }
        if ui.button(fl!("rip-pattern-invert")).clicked() {
            rows.iter_mut().for_each(|row| *row = !*row);
        }
    });
}

pub fn line_editor(ui: &mut egui::Ui, pattern: &mut u16) {
    ui.label(fl!("rip-pattern-line-hint"));
    ui.horizontal(|ui| {
        for bit in (0..16).rev() {
            if ui
                .add(egui::Button::new(" ").selected(*pattern & (1 << bit) != 0).min_size(egui::vec2(18.0, 22.0)))
                .clicked()
            {
                *pattern ^= 1 << bit;
            }
        }
    });
    ui.horizontal(|ui| {
        if ui.button(fl!("rip-pattern-clear")).clicked() {
            *pattern = 0;
        }
        if ui.button(fl!("rip-pattern-invert")).clicked() {
            *pattern = !*pattern;
        }
    });
}
