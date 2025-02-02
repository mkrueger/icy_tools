use std::path::Path;

use directories::UserDirs;

pub mod autosave;

pub fn shorten_directory(mut parent: &Path) -> String {
    if let Some(user) = UserDirs::new() {
        let home_dir = user.home_dir();
        let mut parents = Vec::new();
        let mut root = false;
        while parent != home_dir {
            if let Some(file_name) = parent.file_name() {
                let value = file_name.to_string_lossy().to_string();
                parents.push(value);
                parent = parent.parent().unwrap();
            } else {
                root = true;
                break;
            }
        }
        if root {
            format!("/{}/", parents.into_iter().rev().collect::<Vec<String>>().join("/"))
        } else if parents.is_empty() {
            "~/".to_string()
        } else {
            format!("~/{}/", parents.into_iter().rev().collect::<Vec<String>>().join("/"))
        }
    } else {
        parent.to_string_lossy().to_string()
    }
}
