use iced::{
    Alignment, Color, Element, Length, Theme,
    widget::{Space, column, container, row, scrollable, text},
};
use icy_engine_gui::ui::{
    DIALOG_SPACING, DIALOG_WIDTH_MEDIUM, HEADER_TEXT_SIZE, TEXT_SIZE_NORMAL, button_row_with_left, dialog_area, modal_container, modal_overlay, primary_button,
    secondary_button, separator,
};
use icy_sauce::{ArchiveFormat, AudioFormat, BitmapFormat, Capabilities, CharacterFormat, SauceDataType, SauceRecord, VectorFormat};

const LABEL_WIDTH: f32 = 120.0;

/// SAUCE field colors for dialog - different for light and dark themes
#[derive(Clone, Copy)]
enum SauceFieldColor {
    Title,
    Author,
    Group,
    IceColors,
}

fn sauce_color_style(field: SauceFieldColor) -> impl Fn(&Theme) -> text::Style {
    move |theme: &Theme| {
        let is_dark = theme.extended_palette().is_dark;
        let color = match field {
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
            SauceFieldColor::IceColors => {
                if is_dark {
                    Color::from_rgb(0.4, 0.8, 0.9)
                } else {
                    Color::from_rgb(0.0, 0.5, 0.6)
                }
            }
        };
        text::Style { color: Some(color) }
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

    pub fn view<'a, Message: Clone + 'a>(
        &'a self,
        background: Element<'a, Message>,
        on_message: impl Fn(SauceDialogMessage) -> Message + Copy + 'a,
    ) -> Element<'a, Message> {
        let modal = self.create_modal_content(on_message);
        modal_overlay(background, modal)
    }

    fn section_header<'a, Message: 'a>(title: String) -> Element<'a, Message> {
        text(title)
            .size(HEADER_TEXT_SIZE)
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..iced::Font::default()
            })
            .into()
    }

    fn create_field<'a, Message: 'a>(label: &str, value: &str) -> Element<'a, Message> {
        row![
            text(label.to_string()).size(TEXT_SIZE_NORMAL).width(Length::Fixed(LABEL_WIDTH)),
            text(value.to_string()).size(TEXT_SIZE_NORMAL).style(|theme: &Theme| text::Style {
                color: Some(theme.palette().text.scale_alpha(0.9)),
            }),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center)
        .into()
    }

    fn create_field_with_style<'a, Message: 'a>(label: &str, value: &str, field_color: SauceFieldColor) -> Element<'a, Message> {
        row![
            text(label.to_string()).size(TEXT_SIZE_NORMAL).width(Length::Fixed(LABEL_WIDTH)),
            text(value.to_string()).size(TEXT_SIZE_NORMAL).style(sauce_color_style(field_color)),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center)
        .into()
    }

    // Keep old method for backwards compatibility with direct Color usage
    fn create_field_with_color<'a, Message: 'a>(label: &str, value: &str, color: iced::Color) -> Element<'a, Message> {
        row![
            text(label.to_string()).size(TEXT_SIZE_NORMAL).width(Length::Fixed(LABEL_WIDTH)),
            text(value.to_string()).size(TEXT_SIZE_NORMAL).color(color),
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
            "Unknown".to_string()
        } else {
            date_str.to_string()
        }
    }

    fn create_modal_content<'a, Message: Clone + 'a>(&'a self, on_message: impl Fn(SauceDialogMessage) -> Message + 'a) -> Element<'a, Message> {
        let content = if self.show_raw {
            self.create_raw_content()
        } else {
            self.create_formatted_content()
        };

        // Buttons
        let raw_btn = secondary_button(if self.show_raw { "Formatted" } else { "Raw" }, Some(on_message(SauceDialogMessage::ToggleRaw)));

        let ok_btn = primary_button("OK", Some(on_message(SauceDialogMessage::Close)));

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

    fn create_formatted_content<'a, Message: 'a>(&self) -> Element<'a, Message> {
        let mut sections: Vec<Element<'a, Message>> = Vec::new();

        // Basic info section
        let mut basic_fields: Vec<Element<'a, Message>> = Vec::new();

        let title = self.sauce.title().to_string();
        let title = title.trim();
        if !title.is_empty() {
            basic_fields.push(Self::create_field_with_style("Title:", title, SauceFieldColor::Title));
        }

        let author = self.sauce.author().to_string();
        let author = author.trim();
        if !author.is_empty() {
            basic_fields.push(Self::create_field_with_style("Author:", author, SauceFieldColor::Author));
        }

        let group = self.sauce.group().to_string();
        let group = group.trim();
        if !group.is_empty() {
            basic_fields.push(Self::create_field_with_style("Group:", group, SauceFieldColor::Group));
        }

        basic_fields.push(Self::create_field("Date:", &self.format_date()));

        let data_type = self.sauce.data_type();
        basic_fields.push(Self::create_field("Type:", &format!("{:?}", data_type)));

        basic_fields.push(Self::create_field("File Size:", &format!("{} bytes", self.sauce.file_size())));

        if !basic_fields.is_empty() {
            let mut basic_section = column![
                Self::section_header::<Message>("SAUCE Information".to_string()),
                Space::new().height(DIALOG_SPACING),
            ]
            .spacing(4.0);
            for field in basic_fields {
                basic_section = basic_section.push(field);
            }
            sections.push(basic_section.into());
        }

        // Capabilities section
        if let Some(caps) = self.sauce.capabilities() {
            let caps_content = self.format_capabilities(&caps);
            if !caps_content.is_empty() {
                sections.push(Space::new().height(DIALOG_SPACING).into());
                let mut caps_section = column![Self::section_header::<Message>("Capabilities".to_string()), Space::new().height(DIALOG_SPACING),].spacing(4.0);
                for field in caps_content {
                    caps_section = caps_section.push(field);
                }
                sections.push(caps_section.into());
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
                sections.push(Self::section_header::<Message>("Comments".to_string()));
                sections.push(Space::new().height(4.0).into());
                sections.push(
                    container(
                        scrollable(container(text(comments_text.to_string()).size(TEXT_SIZE_NORMAL)).width(Length::Fill).padding(8))
                            .height(Length::Fixed(80.0))
                            .width(Length::Fill),
                    )
                    .style(container::rounded_box)
                    .into(),
                );
            }
        }

        column(sections).into()
    }

    fn format_capabilities<'a, Message: 'a>(&self, caps: &Capabilities) -> Vec<Element<'a, Message>> {
        let mut fields: Vec<Element<'a, Message>> = Vec::new();

        match caps {
            Capabilities::Character(char_caps) => {
                fields.push(Self::create_field("Format:", &format!("{:?}", char_caps.format)));

                if char_caps.columns > 0 {
                    fields.push(Self::create_field("Columns:", &char_caps.columns.to_string()));
                }
                if char_caps.lines > 0 {
                    fields.push(Self::create_field("Lines:", &char_caps.lines.to_string()));
                }

                if char_caps.ice_colors {
                    fields.push(Self::create_field_with_style("iCE Colors:", "Yes", SauceFieldColor::IceColors));
                }

                if char_caps.letter_spacing.use_letter_spacing() {
                    fields.push(Self::create_field("Letter Spacing:", "9px"));
                }

                if char_caps.aspect_ratio.use_aspect_ratio() {
                    fields.push(Self::create_field("Aspect Ratio:", "Legacy"));
                }

                if let Some(font) = char_caps.font() {
                    let font_str = font.to_string();
                    let font_str = font_str.trim();
                    if !font_str.is_empty() {
                        fields.push(Self::create_field("Font:", font_str));
                    }
                }
            }
            Capabilities::Binary(bin_caps) => {
                fields.push(Self::create_field("Format:", &format!("{:?}", bin_caps.format)));

                if bin_caps.columns > 0 {
                    fields.push(Self::create_field("Columns:", &bin_caps.columns.to_string()));
                }
                if bin_caps.lines > 0 {
                    fields.push(Self::create_field("Lines:", &bin_caps.lines.to_string()));
                }

                if bin_caps.ice_colors {
                    fields.push(Self::create_field_with_style("iCE Colors:", "Yes", SauceFieldColor::IceColors));
                }

                if bin_caps.letter_spacing.use_letter_spacing() {
                    fields.push(Self::create_field("Letter Spacing:", "9px"));
                }

                if bin_caps.aspect_ratio.use_aspect_ratio() {
                    fields.push(Self::create_field("Aspect Ratio:", "Legacy"));
                }

                if let Some(font) = bin_caps.font() {
                    let font_str = font.to_string();
                    let font_str = font_str.trim();
                    if !font_str.is_empty() {
                        fields.push(Self::create_field("Font:", font_str));
                    }
                }
            }
            Capabilities::Vector(vec_caps) => {
                fields.push(Self::create_field("Format:", &format!("{:?}", vec_caps.format)));
            }
            Capabilities::Bitmap(bmp_caps) => {
                fields.push(Self::create_field("Format:", &format!("{:?}", bmp_caps.format)));
                if bmp_caps.width > 0 {
                    fields.push(Self::create_field("Width:", &format!("{}px", bmp_caps.width)));
                }
                if bmp_caps.height > 0 {
                    fields.push(Self::create_field("Height:", &format!("{}px", bmp_caps.height)));
                }
                if bmp_caps.pixel_depth > 0 {
                    fields.push(Self::create_field("Pixel Depth:", &format!("{}bpp", bmp_caps.pixel_depth)));
                }
            }
            Capabilities::Audio(audio_caps) => {
                fields.push(Self::create_field("Format:", &format!("{:?}", audio_caps.format)));
                if audio_caps.sample_rate > 0 {
                    fields.push(Self::create_field("Sample Rate:", &format!("{} Hz", audio_caps.sample_rate)));
                }
            }
            Capabilities::Archive(arch_caps) => {
                fields.push(Self::create_field("Format:", &format!("{:?}", arch_caps.format)));
            }
            Capabilities::Executable(_) => {
                fields.push(Self::create_field("Type:", "Executable"));
            }
        }

        fields
    }

    fn create_raw_content<'a, Message: 'a>(&self) -> Element<'a, Message> {
        let mut fields: Vec<Element<'a, Message>> = Vec::new();

        fields.push(Self::section_header::<Message>("Raw SAUCE Header".to_string()));
        fields.push(Space::new().height(DIALOG_SPACING).into());

        // Display raw header fields
        fields.push(Self::create_field("Title:", self.sauce.title().to_string().trim()));
        fields.push(Self::create_field("Author:", self.sauce.author().to_string().trim()));
        fields.push(Self::create_field("Group:", self.sauce.group().to_string().trim()));
        fields.push(Self::create_field("Date:", &self.sauce.date().to_string()));

        // Get raw header for numeric fields
        let header = self.sauce.header();
        let data_type = self.sauce.data_type();

        // DataType as number with description
        let data_type_num: u8 = header.data_type.into();
        fields.push(Self::create_field("DataType:", &format!("{} ({:?})", data_type_num, data_type)));

        // FileType as number with description
        let file_type_desc = self.get_file_type_description(data_type, header.file_type);
        fields.push(Self::create_field("FileType:", &format!("{} ({})", header.file_type, file_type_desc)));

        fields.push(Self::create_field("FileSize:", &self.sauce.file_size().to_string()));

        fields.push(Space::new().height(DIALOG_SPACING).into());
        fields.push(Self::section_header::<Message>("Technical Info".to_string()));
        fields.push(Space::new().height(DIALOG_SPACING).into());

        // Raw TInfo fields as numbers
        fields.push(Self::create_field("TInfo1:", &header.t_info1.to_string()));
        fields.push(Self::create_field("TInfo2:", &header.t_info2.to_string()));
        fields.push(Self::create_field("TInfo3:", &header.t_info3.to_string()));
        fields.push(Self::create_field("TInfo4:", &header.t_info4.to_string()));

        // TFlags as binary bit field
        fields.push(Self::create_field("TFlags:", &format!("0x{:02X} (0b{:08b})", header.t_flags, header.t_flags)));

        // TInfoS - show the string or "None"
        let tinfos = header.t_info_s.to_string();
        let tinfos = tinfos.trim();
        fields.push(Self::create_field("TInfoS:", if tinfos.is_empty() { "None" } else { tinfos }));

        // Comments count
        let comment_count = self.sauce.comments().len();
        fields.push(Self::create_field("Comments:", &format!("{} lines", comment_count)));

        column(fields).into()
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
