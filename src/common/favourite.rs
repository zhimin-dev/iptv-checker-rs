use std::{fs, io};
use crate::r#const::constant::{FAVOURITE_CONFIG_JSON_CONTENT, FAVOURITE_FILE_NAME};
use crate::utils::file_exists;
use std::io::Write;

pub fn create_favourite_file() {
    if !file_exists(&FAVOURITE_FILE_NAME.to_string()) {
        let mut fd = fs::File::create(FAVOURITE_FILE_NAME.to_string())
            .expect(&format!("Failed to create file: {}", FAVOURITE_FILE_NAME.to_string()));
        fd.write_all(FAVOURITE_CONFIG_JSON_CONTENT.to_string().as_bytes())
            .expect(&format!("Failed to write file: {}", FAVOURITE_FILE_NAME.to_string()));
        fd.flush()
            .expect(&format!("Failed to flush file: {}", FAVOURITE_FILE_NAME.to_string()));
    }
}
