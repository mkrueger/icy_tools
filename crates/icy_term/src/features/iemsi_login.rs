use icy_net::iemsi::IEmsi;

use crate::Res;

pub struct IEmsiAutoLogin {
    pub iemsi: IEmsi,

    pub user_name: String,
    pub password: String,
}

impl IEmsiAutoLogin {
    pub fn new(user_name: String, password: String) -> Self {
        Self {
            iemsi: IEmsi::default(),
            user_name,
            password,
        }
    }
    pub fn is_logged_in(&self) -> bool {
        self.iemsi.logged_in
    }

    pub fn try_login(&mut self, ch: u8) -> Res<Option<Vec<u8>>> {
        Ok(self.iemsi.try_login(&self.user_name, &self.password, ch)?)
    }
}
