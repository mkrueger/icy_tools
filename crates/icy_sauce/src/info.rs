use bstr::{BString, ByteSlice};
use chrono::NaiveDate;

use crate::{
    char_caps::CharCaps,
    header::{SauceHeader, HDR_LEN},
    sauce_pad, sauce_trim, SauceDataType, SauceError, SauceInformationBuilder,
};

pub(crate) const COMMENT_LEN: usize = 64;
const COMMENT_ID_LEN: usize = 5;
const COMMENT_ID: [u8; COMMENT_ID_LEN] = *b"COMNT";

/// For holding SAUCE information which are are altered the meta information
/// can be used to store easily the sauce information without the capabilities.
///
/// This contains all information that are part of SAUCE itself. The rest is information about the file content.
#[derive(Default, Clone, PartialEq)]
pub struct SauceMetaInformation {
    /// The title of the file.
    pub title: BString,
    /// The (nick)name or handle of the creator of the file.
    pub author: BString,
    /// The name of the group or company the creator is employed by.
    pub group: BString,

    pub comments: Vec<BString>,
}

impl SauceMetaInformation {
    pub fn to_builder(&self) -> crate::Result<SauceInformationBuilder> {
        let mut builder = SauceInformationBuilder::default();
        builder = builder.with_title(self.title.clone())?;
        builder = builder.with_author(self.author.clone())?;
        builder = builder.with_group(self.group.clone())?;
        for comment in &self.comments {
            builder = builder.with_comment(comment.clone())?;
        }
        Ok(builder)
    }

    pub fn is_empty(&self) -> bool {
        self.title.is_empty() && self.author.is_empty() && self.group.is_empty() && self.comments.is_empty()
    }
}

/// SAUCE information.
/// This is the main structure for SAUCE.
///
/// SAUCE metadata consits of a header and optional comments.
#[derive(Clone, PartialEq)]
pub struct SauceInformation {
    pub(crate) header: SauceHeader,

    /// Up to 255 comments, each 64 bytes long max.
    pub(crate) comments: Vec<BString>,
}

impl SauceInformation {
    pub fn read(data: &[u8]) -> crate::Result<Option<Self>> {
        let Some(header) = SauceHeader::read(data)? else {
            return Ok(None);
        };

        let mut comments = Vec::new();
        if header.comments > 0 {
            let sauce_len = HDR_LEN + header.comments as usize * COMMENT_LEN + COMMENT_ID_LEN;
            if data.len() < sauce_len {
                return Err(SauceError::InvalidCommentBlock);
            }
            let mut data = &data[data.len() - sauce_len..];

            if COMMENT_ID != data[..COMMENT_ID_LEN] {
                return Err(SauceError::InvalidCommentId(BString::new(data[..5].to_vec())));
            }
            data = &data[COMMENT_ID_LEN..];
            for _ in 0..header.comments {
                let comment = sauce_trim(&data[..COMMENT_LEN]);
                comments.push(comment);
                data = &data[COMMENT_LEN..];
            }
        };

        Ok(Some(SauceInformation { header, comments }))
    }

    pub fn write<A: std::io::Write>(&self, writer: &mut A, file_size: u32) -> crate::Result<()> {
        // EOF Char.
        if let Err(err) = writer.write_all(&[0x1A]) {
            return Err(SauceError::IoError(err));
        }

        if !self.comments.is_empty() {
            let length = COMMENT_ID_LEN + self.comments.len() * COMMENT_LEN;
            let mut comment_info = Vec::with_capacity(length);
            comment_info.extend(&COMMENT_ID);
            for comment in &self.comments {
                comment_info.extend(sauce_pad(comment, COMMENT_LEN));
            }
            assert_eq!(comment_info.len(), length);
            if let Err(err) = writer.write_all(&comment_info) {
                return Err(SauceError::IoError(err));
            }
        }
        self.header.write(writer, file_size)?;
        Ok(())
    }

    /// Returns the byte length of the SAUCE record.
    pub fn info_len(&self) -> usize {
        // +1 for the EOF indicator
        if self.comments.is_empty() {
            HDR_LEN + 1
        } else {
            HDR_LEN + self.header.comments as usize * COMMENT_LEN + COMMENT_ID_LEN + 1
        }
    }

    pub fn title(&self) -> &BString {
        &self.header.title
    }

    pub fn author(&self) -> &BString {
        &self.header.author
    }

    pub fn group(&self) -> &BString {
        &self.header.group
    }

    pub fn get_data_type(&self) -> SauceDataType {
        self.header.data_type
    }

    pub fn comments(&self) -> &[BString] {
        &self.comments
    }

    pub fn get_date(&self) -> crate::Result<NaiveDate> {
        match NaiveDate::parse_from_str(&self.header.date.to_str_lossy(), "%Y%m%d") {
            Ok(d) => Ok(d),
            Err(_) => Err(SauceError::UnsupportedSauceDate(self.header.date.clone())),
        }
    }

    pub fn get_character_capabilities(&self) -> crate::Result<CharCaps> {
        if self.header.data_type != SauceDataType::Character
            && self.header.data_type != SauceDataType::BinaryText
            && self.header.data_type != SauceDataType::XBin
        {
            return Err(SauceError::WrongDataType(self.header.data_type));
        }
        CharCaps::from(&self.header)
    }

    pub fn get_meta_information(&self) -> SauceMetaInformation {
        SauceMetaInformation {
            title: self.header.title.clone(),
            author: self.header.author.clone(),
            group: self.header.group.clone(),
            comments: self.comments.clone(),
        }
    }
}
