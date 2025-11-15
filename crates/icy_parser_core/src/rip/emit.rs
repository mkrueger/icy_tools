use super::*;

impl RipParser {
    pub fn emit_command(&mut self, sink: &mut dyn CommandSink) {
        let cmd = match (self.builder.level, self.builder.cmd_char) {
            // Level 0 commands
            (0, b'w') if self.builder.i32_params.len() >= 5 => RipCommand::TextWindow {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
                wrap: self.builder.i32_params[4] != 0,
                size: *self.builder.i32_params.get(5).unwrap_or(&0),
            },
            (0, b'v') if self.builder.i32_params.len() >= 4 => RipCommand::ViewPort {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
            },
            (0, b'*') => RipCommand::ResetWindows,
            (0, b'e') => RipCommand::EraseWindow,
            (0, b'E') => RipCommand::EraseView,
            (0, b'g') if self.builder.i32_params.len() >= 2 => RipCommand::GotoXY {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
            },
            (0, b'H') => RipCommand::Home,
            (0, b'>') => RipCommand::EraseEOL,
            (0, b'c') if !self.builder.i32_params.is_empty() => RipCommand::Color { c: self.builder.i32_params[0] },
            (0, b'Q') => RipCommand::SetPalette {
                colors: self.builder.i32_params.clone(),
            },
            (0, b'a') if self.builder.i32_params.len() >= 2 => RipCommand::OnePalette {
                color: self.builder.i32_params[0],
                value: self.builder.i32_params[1],
            },
            (0, b'W') if !self.builder.i32_params.is_empty() => RipCommand::WriteMode {
                mode: self.builder.i32_params[0],
            },
            (0, b'm') if self.builder.i32_params.len() >= 2 => RipCommand::Move {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
            },
            (0, b'T') => RipCommand::Text {
                text: self.builder.string_param.clone(),
            },
            (0, b'@') if self.builder.i32_params.len() >= 2 => RipCommand::TextXY {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                text: self.builder.string_param.clone(),
            },
            (0, b'Y') if self.builder.i32_params.len() >= 4 => RipCommand::FontStyle {
                font: self.builder.i32_params[0],
                direction: self.builder.i32_params[1],
                size: self.builder.i32_params[2],
                res: self.builder.i32_params[3],
            },
            (0, b'X') if self.builder.i32_params.len() >= 2 => RipCommand::Pixel {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
            },
            (0, b'L') if self.builder.i32_params.len() >= 4 => RipCommand::Line {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
            },
            (0, b'R') if self.builder.i32_params.len() >= 4 => RipCommand::Rectangle {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
            },
            (0, b'B') if self.builder.i32_params.len() >= 4 => RipCommand::Bar {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
            },
            (0, b'C') if self.builder.i32_params.len() >= 3 => RipCommand::Circle {
                x_center: self.builder.i32_params[0],
                y_center: self.builder.i32_params[1],
                radius: self.builder.i32_params[2],
            },
            (0, b'O') if self.builder.i32_params.len() >= 6 => RipCommand::Oval {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                st_ang: self.builder.i32_params[2],
                end_ang: self.builder.i32_params[3],
                x_rad: self.builder.i32_params[4],
                y_rad: self.builder.i32_params[5],
            },
            (0, b'o') if self.builder.i32_params.len() >= 4 => RipCommand::FilledOval {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                x_rad: self.builder.i32_params[2],
                y_rad: self.builder.i32_params[3],
            },
            (0, b'A') if self.builder.i32_params.len() >= 5 => RipCommand::Arc {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                st_ang: self.builder.i32_params[2],
                end_ang: self.builder.i32_params[3],
                radius: self.builder.i32_params[4],
            },
            (0, b'V') if self.builder.i32_params.len() >= 6 => RipCommand::OvalArc {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                st_ang: self.builder.i32_params[2],
                end_ang: self.builder.i32_params[3],
                x_rad: self.builder.i32_params[4],
                y_rad: self.builder.i32_params[5],
            },
            (0, b'I') if self.builder.i32_params.len() >= 5 => RipCommand::PieSlice {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                st_ang: self.builder.i32_params[2],
                end_ang: self.builder.i32_params[3],
                radius: self.builder.i32_params[4],
            },
            (0, b'i') if self.builder.i32_params.len() >= 6 => RipCommand::OvalPieSlice {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                st_ang: self.builder.i32_params[2],
                end_ang: self.builder.i32_params[3],
                x_rad: self.builder.i32_params[4],
                y_rad: self.builder.i32_params[5],
            },
            (0, b'Z') if self.builder.i32_params.len() >= 9 => RipCommand::Bezier {
                x1: self.builder.i32_params[0],
                y1: self.builder.i32_params[1],
                x2: self.builder.i32_params[2],
                y2: self.builder.i32_params[3],
                x3: self.builder.i32_params[4],
                y3: self.builder.i32_params[5],
                x4: self.builder.i32_params[6],
                y4: self.builder.i32_params[7],
                cnt: self.builder.i32_params[8],
            },
            (0, b'P') => RipCommand::Polygon {
                points: self.builder.i32_params.clone(),
            },
            (0, b'p') => RipCommand::FilledPolygon {
                points: self.builder.i32_params.clone(),
            },
            (0, b'l') => RipCommand::PolyLine {
                points: self.builder.i32_params.clone(),
            },
            (0, b'F') if self.builder.i32_params.len() >= 3 => RipCommand::Fill {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                border: self.builder.i32_params[2],
            },
            (0, b'=') if self.builder.i32_params.len() >= 3 => RipCommand::LineStyle {
                style: self.builder.i32_params[0],
                user_pat: self.builder.i32_params[1],
                thick: self.builder.i32_params[2],
            },
            (0, b'S') if self.builder.i32_params.len() >= 2 => RipCommand::FillStyle {
                pattern: self.builder.i32_params[0],
                color: self.builder.i32_params[1],
            },
            (0, b's') if self.builder.i32_params.len() >= 9 => RipCommand::FillPattern {
                c1: self.builder.i32_params[0],
                c2: self.builder.i32_params[1],
                c3: self.builder.i32_params[2],
                c4: self.builder.i32_params[3],
                c5: self.builder.i32_params[4],
                c6: self.builder.i32_params[5],
                c7: self.builder.i32_params[6],
                c8: self.builder.i32_params[7],
                col: self.builder.i32_params[8],
            },
            (0, b'$') => RipCommand::TextVariable {
                text: self.builder.string_param.clone(),
            },
            (0, b'#') => RipCommand::NoMore,

            // Level 1 commands
            (1, b'M') if self.builder.i32_params.len() >= 8 => RipCommand::Mouse {
                num: self.builder.i32_params[0],
                x0: self.builder.i32_params[1],
                y0: self.builder.i32_params[2],
                x1: self.builder.i32_params[3],
                y1: self.builder.i32_params[4],
                clk: self.builder.i32_params[5],
                clr: self.builder.i32_params[6],
                res: self.builder.i32_params[7],
                text: self.builder.string_param.clone(),
            },
            (1, b'K') => RipCommand::MouseFields,
            (1, b'T') if self.builder.i32_params.len() >= 5 => RipCommand::BeginText {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
                res: self.builder.i32_params[4],
            },
            (1, b't') => RipCommand::RegionText {
                justify: !self.builder.i32_params.is_empty() && self.builder.i32_params[0] != 0,
                text: self.builder.string_param.clone(),
            },
            (1, b'E') => RipCommand::EndText,
            (1, b'C') if self.builder.i32_params.len() >= 5 => RipCommand::GetImage {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
                res: self.builder.i32_params[4],
            },
            (1, b'P') if self.builder.i32_params.len() >= 4 => RipCommand::PutImage {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                mode: self.builder.i32_params[2],
                res: self.builder.i32_params[3],
            },
            (1, b'W') => RipCommand::WriteIcon {
                res: self.builder.char_param,
                data: self.builder.string_param.clone(),
            },
            (1, b'I') if self.builder.i32_params.len() >= 5 => RipCommand::LoadIcon {
                x: self.builder.i32_params[0],
                y: self.builder.i32_params[1],
                mode: self.builder.i32_params[2],
                clipboard: self.builder.i32_params[3],
                res: self.builder.i32_params[4],
                file_name: self.builder.string_param.clone(),
            },
            (1, b'B') if self.builder.i32_params.len() >= 15 => RipCommand::ButtonStyle {
                wid: self.builder.i32_params[0],
                hgt: self.builder.i32_params[1],
                orient: self.builder.i32_params[2],
                flags: self.builder.i32_params[3],
                bevsize: self.builder.i32_params[4],
                dfore: self.builder.i32_params[5],
                dback: self.builder.i32_params[6],
                bright: self.builder.i32_params[7],
                dark: self.builder.i32_params[8],
                surface: self.builder.i32_params[9],
                grp_no: self.builder.i32_params[10],
                flags2: self.builder.i32_params[11],
                uline_col: self.builder.i32_params[12],
                corner_col: self.builder.i32_params[13],
                res: self.builder.i32_params[14],
            },
            (1, b'U') if self.builder.i32_params.len() >= 7 => RipCommand::Button {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
                hotkey: self.builder.i32_params[4],
                flags: self.builder.i32_params[5],
                res: self.builder.i32_params[6],
                text: self.builder.string_param.clone(),
            },
            (1, b'D') if self.builder.i32_params.len() >= 2 => RipCommand::Define {
                flags: self.builder.i32_params[0],
                res: self.builder.i32_params[1],
                text: self.builder.string_param.clone(),
            },
            (1, 0x1B) if self.builder.i32_params.len() >= 2 => RipCommand::Query {
                mode: self.builder.i32_params[0],
                res: self.builder.i32_params[1],
                text: self.builder.string_param.clone(),
            },
            (1, b'G') if self.builder.i32_params.len() >= 6 => RipCommand::CopyRegion {
                x0: self.builder.i32_params[0],
                y0: self.builder.i32_params[1],
                x1: self.builder.i32_params[2],
                y1: self.builder.i32_params[3],
                res: self.builder.i32_params[4],
                dest_line: self.builder.i32_params[5],
            },
            (1, b'R') => RipCommand::ReadScene {
                file_name: self.builder.string_param.clone(),
            },
            (1, b'F') => RipCommand::FileQuery {
                file_name: self.builder.string_param.clone(),
            },

            // Level 9 commands
            (9, 0x1B) if self.builder.i32_params.len() >= 4 => RipCommand::EnterBlockMode {
                mode: self.builder.i32_params[0],
                proto: self.builder.i32_params[1],
                file_type: self.builder.i32_params[2],
                res: self.builder.i32_params[3],
                file_name: self.builder.string_param.clone(),
            },

            _ => {
                // Unknown command - don't emit anything
                return;
            }
        };

        sink.emit_rip(cmd);
    }
}
