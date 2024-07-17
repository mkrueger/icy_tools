use bstr::{BString, ByteSlice};

use crate::{sauce_pad, sauce_trim, zero_pad, SauceDataType, SauceError};

pub(crate) const HDR_LEN: usize = 128;
const SAUCE_ID: &[u8; 5] = b"SAUCE";
pub(crate) const TITLE_LEN: usize = 35;
pub(crate) const AUTHOR_GROUP_LEN: usize = 20;
pub(crate) const TINFO_LEN: usize = 22;

#[derive(Clone, Default, PartialEq)]
pub struct SauceHeader {
    /// begins with b"SAUCE"
    /// The title of the file.
    pub title: BString,
    /// The (nick)name or handle of the creator of the file.
    pub author: BString,
    /// The name of the group or company the creator is employed by.
    pub group: BString,

    /// The format for the date is CCYYMMDD (century, year, month, day).
    pub date: BString,

    /// Type of data.
    pub data_type: SauceDataType,

    /// Type is variable depending on the data_type.
    pub file_type: u8,

    /// Type dependant numeric information field 1.
    pub t_info1: u16,
    /// Type dependant numeric information field 2.
    pub t_info2: u16,
    /// Type dependant numeric information field 3.
    pub t_info3: u16,
    /// Type dependant numeric information field 4.
    pub t_info4: u16,

    /// Number of lines in the extra SAUCE comment block.
    /// 0 indicates no comment block is present.
    pub comments: u8,

    /// Type dependant flags.
    pub t_flags: u8,

    /// Type dependant string information field
    pub t_info_s: BString,
}

impl SauceHeader {
    /// .
    ///
    /// # Panics
    /// # Errors
    ///
    /// This function will return an error if the file con
    pub fn read(data: &[u8]) -> crate::Result<Option<Self>> {
        if data.len() < HDR_LEN {
            return Ok(None);
        }

        let mut data = &data[data.len() - HDR_LEN..];
        if SAUCE_ID != &data[..5] {
            return Ok(None);
        }
        data = &data[5..];

        if b"00" != &data[0..2] {
            return Err(SauceError::UnsupportedSauceVersion(BString::new(data[0..2].to_vec())));
        }
        data = &data[2..];

        let title = sauce_trim(&data[0..TITLE_LEN]);
        data = &data[TITLE_LEN..];
        let author = sauce_trim(&data[0..AUTHOR_GROUP_LEN]);
        data = &data[AUTHOR_GROUP_LEN..];
        let group = sauce_trim(&data[0..AUTHOR_GROUP_LEN]);
        data = &data[AUTHOR_GROUP_LEN..];

        let creation_date = BString::new(data[0..8].to_vec());

        // skip file_size - we can calculate it, better than to rely on random 3rd party software.
        // Question: are there files where that is important?
        data = &data[8 + 4..];

        let data_type = SauceDataType::from(data[0]);
        let file_type = data[1];
        let t_info1 = data[2] as u16 + ((data[3] as u16) << 8);
        let t_info2 = data[4] as u16 + ((data[5] as u16) << 8);
        let t_info3 = data[6] as u16 + ((data[7] as u16) << 8);
        let t_info4 = data[8] as u16 + ((data[9] as u16) << 8);
        let num_comments = data[10];
        let t_flags = data[11];
        data = &data[12..];

        assert_eq!(data.len(), TINFO_LEN);
        let t_info_s = sauce_trim(data);
        Ok(Some(Self {
            title,
            author,
            group,
            date: creation_date,
            data_type,
            file_type,
            t_info1,
            t_info2,
            t_info3,
            t_info4,
            comments: num_comments,
            t_flags,
            t_info_s,
        }))
    }

    /// Writes the SAUCE header to a writer.
    /// Note that file_size can't be determined by the header alone, so it must be provided.
    /// Comments are written prior to the header.
    pub fn write<A: std::io::Write>(&self, writer: &mut A, file_size: u32) -> crate::Result<()> {
        let mut sauce_info = Vec::with_capacity(HDR_LEN);
        sauce_info.extend(SAUCE_ID);
        sauce_info.extend(b"00");
        sauce_info.extend(sauce_pad(&self.title, TITLE_LEN));
        sauce_info.extend(sauce_pad(&self.author, AUTHOR_GROUP_LEN));
        sauce_info.extend(sauce_pad(&self.group, AUTHOR_GROUP_LEN));
        sauce_info.extend(sauce_pad(&self.date, 8));
        sauce_info.extend(file_size.to_le_bytes());
        sauce_info.push(self.data_type.into());
        sauce_info.push(self.file_type);
        sauce_info.extend(&self.t_info1.to_le_bytes());
        sauce_info.extend(&self.t_info2.to_le_bytes());
        sauce_info.extend(&self.t_info3.to_le_bytes());
        sauce_info.extend(&self.t_info4.to_le_bytes());
        sauce_info.push(self.comments);
        sauce_info.push(self.t_flags);
        sauce_info.extend(zero_pad(&self.t_info_s, TINFO_LEN));

        assert_eq!(sauce_info.len(), HDR_LEN);

        if let Err(err) = writer.write_all(&sauce_info) {
            return Err(SauceError::IoError(err));
        }
        Ok(())
    }
}
