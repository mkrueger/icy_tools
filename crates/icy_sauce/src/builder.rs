use bstr::BString;
use chrono::NaiveDate;

use crate::{
    COMMENT_LEN, SauceDataType, SauceError,
    char_caps::CharCaps,
    header::{AUTHOR_GROUP_LEN, SauceHeader, TITLE_LEN},
};

/// The builder helps creating valid SAUCE records
#[derive(Default)]
pub struct SauceInformationBuilder {
    header: SauceHeader,

    /// Up to 255 comments, each 64 bytes long max.
    comments: Vec<BString>,
}

impl SauceInformationBuilder {
    pub fn with_title(mut self, title: BString) -> crate::Result<Self> {
        if title.len() > TITLE_LEN {
            return Err(SauceError::TitleTooLong(title.len()));
        }
        self.header.title = title;
        Ok(self)
    }

    pub fn with_author(mut self, author: BString) -> crate::Result<Self> {
        if author.len() > AUTHOR_GROUP_LEN {
            return Err(SauceError::AuthorTooLong(author.len()));
        }
        self.header.author = author;
        Ok(self)
    }

    pub fn with_group(mut self, group: BString) -> crate::Result<Self> {
        if group.len() > AUTHOR_GROUP_LEN {
            return Err(SauceError::GroupTooLong(group.len()));
        }
        self.header.group = group;
        Ok(self)
    }

    pub fn with_date(mut self, date: NaiveDate) -> Self {
        self.header.date = date.format("%Y%m%d").to_string().into();
        self
    }

    pub fn with_data_type(mut self, data_type: SauceDataType) -> Self {
        self.header.data_type = data_type;
        self
    }

    pub fn with_char_caps(mut self, caps: CharCaps) -> crate::Result<Self> {
        if self.header.data_type != SauceDataType::Character
            && self.header.data_type != SauceDataType::XBin
            && self.header.data_type != SauceDataType::BinaryText
        {
            return Err(SauceError::WrongDataType(self.header.data_type));
        }
        caps.write_to_header(&mut self.header);
        Ok(self)
    }

    /// Adds a comment to the SAUCE record
    pub fn with_comment(mut self, comment: BString) -> crate::Result<Self> {
        if self.comments.len() >= 255 {
            return Err(SauceError::CommentLimitExceeded);
        }
        if comment.len() > COMMENT_LEN {
            return Err(SauceError::CommentTooLong(comment.len()));
        }
        self.comments.push(comment);
        self.header.comments = self.comments.len() as u8;
        Ok(self)
    }

    /// Builds the SAUCE record
    pub fn build(self) -> crate::SauceInformation {
        crate::SauceInformation {
            header: self.header,
            comments: self.comments,
        }
    }
}
