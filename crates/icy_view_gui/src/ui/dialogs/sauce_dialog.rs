use i18n_embed_fl::fl;
use iced::{
    Alignment, Color, Element, Length, Theme,
    widget::{Space, column, container, row, scrollable, text, text_input},
};
use icy_engine_gui::{
    section_header,
    ui::{
        DIALOG_SPACING, DIALOG_WIDTH_MEDIUM, TEXT_SIZE_NORMAL, button_row_with_left, dialog_area, left_label_small, modal_container, modal_overlay,
        primary_button, secondary_button, separator,
    },
};
use icy_sauce::{ArchiveFormat, AudioFormat, BitmapFormat, Capabilities, CharacterFormat, SauceDataType, SauceRecord, VectorFormat};

const FIELD_SPACING: f32 = 4.0;

/// SAUCE field colors for dialog - different for light and dark themes
#[derive(Clone, Copy)]
enum SauceFieldColor {
    Title,
    Author,
    Group,
    Normal,
}

fn get_sauce_color(field: SauceFieldColor, theme: &Theme) -> Color {
    let is_dark = theme.extended_palette().is_dark;
    match field {
        SauceFieldColor::Title => {
            if is_dark {
                Color::from_rgb(0.9, 0.9, 0.6)
            } else {
                Color::from_rgb(0.6, 0.5, 0.0)
            }
        }
        SauceFieldColor::Author => {
            if is_dark {
                Color::from_rgb(0.6, 0.9, 0.6)
            } else {
                Color::from_rgb(0.0, 0.5, 0.0)
            }
        }
        SauceFieldColor::Group => {
            if is_dark {
                Color::from_rgb(0.6, 0.8, 0.9)
            } else {
                Color::from_rgb(0.0, 0.4, 0.6)
            }
        }
        SauceFieldColor::Normal => theme.palette().text,
    }
}

fn sauce_input_style(field: SauceFieldColor) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |theme: &Theme, _status: text_input::Status| {
        let palette = theme.extended_palette();
        let value_color = get_sauce_color(field, theme);
        text_input::Style {
            background: iced::Background::Color(palette.background.weak.color),
            border: iced::Border {
                color: palette.background.strong.color,
                width: 1.0,
                radius: 4.0.into(),
            },
            icon: palette.background.strong.text.scale_alpha(0.6),
            placeholder: palette.background.base.text.scale_alpha(0.5),
            value: value_color,
            selection: palette.primary.weak.color.scale_alpha(0.5),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SauceDialogMessage {
    Close,
    ToggleRaw,
}

pub struct SauceDialog {
    sauce: SauceRecord,
    show_raw: bool,
}

impl SauceDialog {
    pub fn new(sauce: SauceRecord) -> Self {
        Self { sauce, show_raw: false }
    }

    pub fn update(&mut self, message: SauceDialogMessage) -> bool {
        match message {
            SauceDialogMessage::Close => true,
            SauceDialogMessage::ToggleRaw => {
                self.show_raw = !self.show_raw;
                false
            }
        }
    }

    pub fn view<'a, Message: Clone + 'static>(
        &'a self,
        background: Element<'a, Message>,
        on_message: impl Fn(SauceDialogMessage) -> Message + Copy + 'a,
    ) -> Element<'a, Message> {
        let modal = self.create_modal_content(on_message);
        modal_overlay(background, modal)
    }

    fn create_field<'a, Message: Clone + 'static>(label: &str, value: &str) -> Element<'a, Message> {
        row![
            left_label_small(label.to_string()),
            text_input("", value)
                .size(TEXT_SIZE_NORMAL)
                .width(Length::Fill)
                .style(sauce_input_style(SauceFieldColor::Normal)),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center)
        .into()
    }

    fn create_field_with_style<'a, Message: Clone + 'static>(label: &str, value: &str, field_color: SauceFieldColor) -> Element<'a, Message> {
        row![
            left_label_small(label.to_string()),
            text_input("", value)
                .size(TEXT_SIZE_NORMAL)
                .width(Length::Fill)
                .style(sauce_input_style(field_color)),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center)
        .into()
    }

    fn format_date(&self) -> String {
        let date = self.sauce.date();
        let date_str = date.to_string();
        let date_str = date_str.trim();
        if date_str.len() == 8 && date_str != "00000000" {
            format!("{}-{}-{}", &date_str[0..4], &date_str[4..6], &date_str[6..8])
        } else if date_str.is_empty() || date_str == "00000000" {
            fl!(crate::LANGUAGE_LOADER, "sauce-unknown")
        } else {
            date_str.to_string()
        }
    }

    fn create_modal_content<'a, Message: Clone + 'static>(&'a self, on_message: impl Fn(SauceDialogMessage) -> Message + 'a) -> Element<'a, Message> {
        let content = if self.show_raw {
            self.create_raw_content()
        } else {
            self.create_formatted_content()
        };

        // Buttons
        let raw_btn = secondary_button(
            if self.show_raw {
                fl!(crate::LANGUAGE_LOADER, "sauce-btn-formatted")
            } else {
                fl!(crate::LANGUAGE_LOADER, "sauce-btn-raw")
            },
            Some(on_message(SauceDialogMessage::ToggleRaw)),
        );

        let ok_btn = primary_button(fl!(crate::LANGUAGE_LOADER, "button-ok"), Some(on_message(SauceDialogMessage::Close)));

        let buttons = button_row_with_left(vec![raw_btn.into()], vec![ok_btn.into()]);

        let dialog_content = dialog_area(content);
        let button_area = dialog_area(buttons);

        let modal = modal_container(
            column![container(dialog_content).height(Length::Shrink), separator(), button_area,].into(),
            DIALOG_WIDTH_MEDIUM,
        );

        container(modal)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }

    fn create_comments_box<'a, Message: Clone + 'static>(comments_text: &str) -> Element<'a, Message> {
        container(
            scrollable(container(text(comments_text.to_string()).size(TEXT_SIZE_NORMAL)).width(Length::Fill).padding(6))
                .height(Length::Fixed(80.0))
                .width(Length::Fill),
        )
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(iced::Background::Color(palette.background.weak.color)),
                border: iced::Border {
                    color: palette.background.strong.color,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            }
        })
        .into()
    }

    fn create_formatted_content<'a, Message: Clone + 'static>(&self) -> Element<'a, Message> {
        let mut sections: Vec<Element<'a, Message>> = Vec::new();

        // Basic info section
        let mut basic_fields: Vec<Element<'a, Message>> = Vec::new();

        let title = self.sauce.title().to_string();
        let title = title.trim();
        if !title.is_empty() {
            basic_fields.push(Self::create_field_with_style(
                &fl!(crate::LANGUAGE_LOADER, "sauce-field-title"),
                title,
                SauceFieldColor::Title,
            ));
        }

        let author = self.sauce.author().to_string();
        let author = author.trim();
        if !author.is_empty() {
            basic_fields.push(Self::create_field_with_style(
                &fl!(crate::LANGUAGE_LOADER, "sauce-field-author"),
                author,
                SauceFieldColor::Author,
            ));
        }

        let group = self.sauce.group().to_string();
        let group = group.trim();
        if !group.is_empty() {
            basic_fields.push(Self::create_field_with_style(
                &fl!(crate::LANGUAGE_LOADER, "sauce-field-group"),
                group,
                SauceFieldColor::Group,
            ));
        }

        basic_fields.push(Self::create_field(&fl!(crate::LANGUAGE_LOADER, "sauce-field-date"), &self.format_date()));

        basic_fields.push(Self::create_field(
            &fl!(crate::LANGUAGE_LOADER, "sauce-field-file-size"),
            &fl!(crate::LANGUAGE_LOADER, "sauce-value-bytes", count = self.sauce.file_size()),
        ));

        if !basic_fields.is_empty() {
            sections.push(section_header(fl!(crate::LANGUAGE_LOADER, "sauce-section-info")));
            let mut basic_content = column![].spacing(FIELD_SPACING);
            for field in basic_fields {
                basic_content = basic_content.push(field);
            }
            sections.push(basic_content.into());
        }

        // Capabilities section
        if let Some(caps) = self.sauce.capabilities() {
            let data_type = self.sauce.data_type();
            let caps_content = self.format_capabilities(&caps, data_type);
            if !caps_content.is_empty() {
                sections.push(Space::new().height(DIALOG_SPACING).into());
                sections.push(section_header(fl!(crate::LANGUAGE_LOADER, "sauce-section-capabilities")));
                let mut caps_column = column![].spacing(FIELD_SPACING);
                for field in caps_content {
                    caps_column = caps_column.push(field);
                }
                sections.push(caps_column.into());
            }
        }

        // Comments section
        let comments = self.sauce.comments();
        if !comments.is_empty() {
            let mut comments_text = String::new();
            for comment in comments {
                let s = comment.to_string();
                if !s.trim().is_empty() {
                    comments_text.push_str(&s);
                    comments_text.push('\n');
                }
            }
            let comments_text = comments_text.trim();
            if !comments_text.is_empty() {
                sections.push(Space::new().height(DIALOG_SPACING).into());
                sections.push(section_header(fl!(crate::LANGUAGE_LOADER, "sauce-section-comments")));
                sections.push(Self::create_comments_box(comments_text));
            }
        }

        column(sections).into()
    }

    fn format_capabilities<'a, Message: Clone + 'static>(&self, caps: &Capabilities, data_type: SauceDataType) -> Vec<Element<'a, Message>> {
        let mut fields: Vec<Element<'a, Message>> = Vec::new();
        let type_str = format!("{:?}", data_type);

        match caps {
            Capabilities::Character(char_caps) => {
                // Combine type and format: "Character / Ansi"
                let format_str = format!("{} / {:?}", type_str, char_caps.format);
                fields.push(Self::create_field(&fl!(crate::LANGUAGE_LOADER, "sauce-field-format"), &format_str));

                // Combine columns and lines into one "Size" field
                if char_caps.columns > 0 || char_caps.lines > 0 {
                    let size_str = format!("{}x{}", char_caps.columns, char_caps.lines);
                    fields.push(Self::create_field(&fl!(crate::LANGUAGE_LOADER, "sauce-field-size"), &size_str));
                }

                // Combine iCE Colors, letter spacing and aspect ratio into one "Flags" field
                let mut flags = Vec::new();
                if char_caps.ice_colors {
                    flags.push("iCE".to_string());
                }
                let letter_spacing = if char_caps.letter_spacing.use_letter_spacing() {
                    fl!(crate::LANGUAGE_LOADER, "sauce-value-9px")
                } else {
                    fl!(crate::LANGUAGE_LOADER, "sauce-value-8px")
                };
                flags.push(letter_spacing);
                let aspect_ratio = if char_caps.aspect_ratio.use_aspect_ratio() {
                    fl!(crate::LANGUAGE_LOADER, "sauce-value-legacy")
                } else {
                    fl!(crate::LANGUAGE_LOADER, "sauce-value-modern")
                };
                flags.push(aspect_ratio);
                fields.push(Self::create_field(&fl!(crate::LANGUAGE_LOADER, "sauce-field-flags"), &flags.join(", ")));

                if let Some(font) = char_caps.font() {
                    let font_str = font.to_string();
                    let font_str = font_str.trim();
                    if !font_str.is_empty() {
                        fields.push(Self::create_field(&fl!(crate::LANGUAGE_LOADER, "sauce-field-font"), font_str));
                    }
                }
            }
            Capabilities::Binary(bin_caps) => {
                // Combine type and format: "Binary / Xbin"
                let format_str = format!("{} / {:?}", type_str, bin_caps.format);
                fields.push(Self::create_field(&fl!(crate::LANGUAGE_LOADER, "sauce-field-format"), &format_str));

                // Combine columns and lines into one "Size" field
                if bin_caps.columns > 0 || bin_caps.lines > 0 {
                    let size_str = format!("{}x{}", bin_caps.columns, bin_caps.lines);
                    fields.push(Self::create_field(&fl!(crate::LANGUAGE_LOADER, "sauce-field-size"), &size_str));
                }

                // Combine iCE Colors, letter spacing and aspect ratio into one "Flags" field
                let mut flags = Vec::new();
                if bin_caps.ice_colors {
                    flags.push("iCE".to_string());
                }
                let letter_spacing = if bin_caps.letter_spacing.use_letter_spacing() {
                    fl!(crate::LANGUAGE_LOADER, "sauce-value-9px")
                } else {
                    fl!(crate::LANGUAGE_LOADER, "sauce-value-8px")
                };
                flags.push(letter_spacing);
                let aspect_ratio = if bin_caps.aspect_ratio.use_aspect_ratio() {
                    fl!(crate::LANGUAGE_LOADER, "sauce-value-legacy")
                } else {
                    fl!(crate::LANGUAGE_LOADER, "sauce-value-modern")
                };
                flags.push(aspect_ratio);
                fields.push(Self::create_field(&fl!(crate::LANGUAGE_LOADER, "sauce-field-flags"), &flags.join(", ")));

                if let Some(font) = bin_caps.font() {
                    let font_str = font.to_string();
                    let font_str = font_str.trim();
                    if !font_str.is_empty() {
                        fields.push(Self::create_field(&fl!(crate::LANGUAGE_LOADER, "sauce-field-font"), font_str));
                    }
                }
            }
            Capabilities::Vector(vec_caps) => {
                // Combine type and format
                let format_str = format!("{} / {:?}", type_str, vec_caps.format);
                fields.push(Self::create_field(&fl!(crate::LANGUAGE_LOADER, "sauce-field-format"), &format_str));
            }
            Capabilities::Bitmap(bmp_caps) => {
                // Combine type and format
                let format_str = format!("{} / {:?}", type_str, bmp_caps.format);
                fields.push(Self::create_field(&fl!(crate::LANGUAGE_LOADER, "sauce-field-format"), &format_str));
                if bmp_caps.width > 0 {
                    fields.push(Self::create_field(
                        &fl!(crate::LANGUAGE_LOADER, "sauce-field-width"),
                        &fl!(crate::LANGUAGE_LOADER, "sauce-value-pixels", count = bmp_caps.width),
                    ));
                }
                if bmp_caps.height > 0 {
                    fields.push(Self::create_field(
                        &fl!(crate::LANGUAGE_LOADER, "sauce-field-height"),
                        &fl!(crate::LANGUAGE_LOADER, "sauce-value-pixels", count = bmp_caps.height),
                    ));
                }
                if bmp_caps.pixel_depth > 0 {
                    fields.push(Self::create_field(
                        &fl!(crate::LANGUAGE_LOADER, "sauce-field-pixel-depth"),
                        &fl!(crate::LANGUAGE_LOADER, "sauce-value-bpp", count = bmp_caps.pixel_depth),
                    ));
                }
            }
            Capabilities::Audio(audio_caps) => {
                // Combine type and format
                let format_str = format!("{} / {:?}", type_str, audio_caps.format);
                fields.push(Self::create_field(&fl!(crate::LANGUAGE_LOADER, "sauce-field-format"), &format_str));
                if audio_caps.sample_rate > 0 {
                    fields.push(Self::create_field(
                        &fl!(crate::LANGUAGE_LOADER, "sauce-field-sample-rate"),
                        &fl!(crate::LANGUAGE_LOADER, "sauce-value-hz", count = audio_caps.sample_rate),
                    ));
                }
            }
            Capabilities::Archive(arch_caps) => {
                // Combine type and format
                let format_str = format!("{} / {:?}", type_str, arch_caps.format);
                fields.push(Self::create_field(&fl!(crate::LANGUAGE_LOADER, "sauce-field-format"), &format_str));
            }
            Capabilities::Executable(_) => {
                // Just show the type for executables
                fields.push(Self::create_field(&fl!(crate::LANGUAGE_LOADER, "sauce-field-format"), &type_str));
            }
        }

        fields
    }

    fn create_raw_content<'a, Message: Clone + 'static>(&self) -> Element<'a, Message> {
        let mut sections: Vec<Element<'a, Message>> = Vec::new();

        sections.push(section_header(fl!(crate::LANGUAGE_LOADER, "sauce-section-raw-header")));

        // Display raw header fields
        let mut header_fields: Vec<Element<'a, Message>> = Vec::new();
        header_fields.push(Self::create_field(
            &fl!(crate::LANGUAGE_LOADER, "sauce-field-title"),
            self.sauce.title().to_string().trim(),
        ));
        header_fields.push(Self::create_field(
            &fl!(crate::LANGUAGE_LOADER, "sauce-field-author"),
            self.sauce.author().to_string().trim(),
        ));
        header_fields.push(Self::create_field(
            &fl!(crate::LANGUAGE_LOADER, "sauce-field-group"),
            self.sauce.group().to_string().trim(),
        ));
        header_fields.push(Self::create_field(
            &fl!(crate::LANGUAGE_LOADER, "sauce-field-date"),
            &self.sauce.date().to_string(),
        ));

        // Get raw header for numeric fields
        let header = self.sauce.header();
        let data_type = self.sauce.data_type();

        // DataType as number with description
        let data_type_num: u8 = header.data_type.into();
        header_fields.push(Self::create_field(
            &fl!(crate::LANGUAGE_LOADER, "sauce-field-data-type"),
            &format!("{} ({:?})", data_type_num, data_type),
        ));

        // FileType as number with description
        let file_type_desc = self.get_file_type_description(data_type, header.file_type);
        header_fields.push(Self::create_field(
            &fl!(crate::LANGUAGE_LOADER, "sauce-field-file-type"),
            &format!("{} ({})", header.file_type, file_type_desc),
        ));

        header_fields.push(Self::create_field(
            &fl!(crate::LANGUAGE_LOADER, "sauce-field-file-size"),
            &self.sauce.file_size().to_string(),
        ));

        let mut header_column = column![].spacing(FIELD_SPACING);
        for field in header_fields {
            header_column = header_column.push(field);
        }
        sections.push(header_column.into());

        sections.push(Space::new().height(DIALOG_SPACING).into());
        sections.push(section_header(fl!(crate::LANGUAGE_LOADER, "sauce-section-technical")));

        // Raw TInfo fields as numbers
        let mut tech_fields: Vec<Element<'a, Message>> = Vec::new();
        tech_fields.push(Self::create_field(
            &fl!(crate::LANGUAGE_LOADER, "sauce-field-tinfo1"),
            &header.t_info1.to_string(),
        ));
        tech_fields.push(Self::create_field(
            &fl!(crate::LANGUAGE_LOADER, "sauce-field-tinfo2"),
            &header.t_info2.to_string(),
        ));
        tech_fields.push(Self::create_field(
            &fl!(crate::LANGUAGE_LOADER, "sauce-field-tinfo3"),
            &header.t_info3.to_string(),
        ));
        tech_fields.push(Self::create_field(
            &fl!(crate::LANGUAGE_LOADER, "sauce-field-tinfo4"),
            &header.t_info4.to_string(),
        ));

        // TFlags as binary bit field
        tech_fields.push(Self::create_field(
            &fl!(crate::LANGUAGE_LOADER, "sauce-field-tflags"),
            &format!("0x{:02X} (0b{:08b})", header.t_flags, header.t_flags),
        ));

        // TInfoS - show the string or "None"
        let tinfos = header.t_info_s.to_string();
        let tinfos = tinfos.trim();
        let tinfos_display = if tinfos.is_empty() {
            fl!(crate::LANGUAGE_LOADER, "sauce-value-none")
        } else {
            tinfos.to_string()
        };
        tech_fields.push(Self::create_field(&fl!(crate::LANGUAGE_LOADER, "sauce-field-tinfos"), &tinfos_display));

        // Comments count
        let comment_count = self.sauce.comments().len();
        tech_fields.push(Self::create_field(
            &fl!(crate::LANGUAGE_LOADER, "sauce-section-comments"),
            &fl!(crate::LANGUAGE_LOADER, "sauce-value-lines", count = comment_count),
        ));

        let mut tech_column = column![].spacing(FIELD_SPACING);
        for field in tech_fields {
            tech_column = tech_column.push(field);
        }
        sections.push(tech_column.into());

        // Comments content in raw mode too
        let comments = self.sauce.comments();
        if !comments.is_empty() {
            let mut comments_text = String::new();
            for comment in comments {
                let s = comment.to_string();
                comments_text.push_str(&s);
                comments_text.push('\n');
            }
            let comments_text = comments_text.trim_end();
            if !comments_text.is_empty() {
                sections.push(Space::new().height(DIALOG_SPACING).into());
                sections.push(section_header(fl!(crate::LANGUAGE_LOADER, "sauce-section-comment-lines")));
                sections.push(Self::create_comments_box(comments_text));
            }
        }

        column(sections).into()
    }

    fn get_file_type_description(&self, data_type: SauceDataType, file_type: u8) -> String {
        match data_type {
            SauceDataType::None => "None".to_string(),
            SauceDataType::Character => format!("{:?}", CharacterFormat::from_sauce(file_type)),
            SauceDataType::Bitmap => format!("{:?}", BitmapFormat::from_sauce(data_type, file_type)),
            SauceDataType::Vector => format!("{:?}", VectorFormat::from_sauce(file_type)),
            SauceDataType::Audio => format!("{:?}", AudioFormat::from_sauce(file_type)),
            SauceDataType::BinaryText => "BinaryText".to_string(),
            SauceDataType::XBin => "XBin".to_string(),
            SauceDataType::Archive => format!("{:?}", ArchiveFormat::from_sauce(file_type)),
            SauceDataType::Executable => "Executable".to_string(),
            SauceDataType::Undefined(v) => format!("Undefined({})", v),
        }
    }
}
