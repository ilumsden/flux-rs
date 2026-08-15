use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use nix::sys::stat::{Mode, SFlag, lstat};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{FluxError, Result};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilerefEncoding {
    #[serde(rename = "utf-8")]
    Utf8,
    #[serde(rename = "base64")]
    Base64,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum FilerefData {
    Encoded(Vec<u8>),
    Text(String),
    Json(Value),
}

fn default_mode() -> u32 {
    let permissions = Mode::S_IRUSR | Mode::S_IWUSR;
    let file_type = SFlag::S_IFREG;
    permissions.bits() | file_type.bits()
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Fileref {
    pub path: PathBuf,
    #[serde(default = "default_mode")]
    pub mode: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mtime: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ctime: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<FilerefEncoding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<FilerefData>,
}

impl Fileref {
    pub fn new(
        path: impl AsRef<Path>,
        mode: Option<Mode>,
        mtime: Option<u64>,
        ctime: Option<u64>,
        size: Option<usize>,
        encoding: Option<FilerefEncoding>,
        data: Option<FilerefData>,
    ) -> Self {
        let real_mode = mode.map_or_else(default_mode, |m| m.bits()) | SFlag::S_IFREG.bits();
        Self {
            path: path.as_ref().to_path_buf(),
            mode: real_mode,
            mtime,
            ctime,
            size,
            encoding,
            data,
        }
    }

    pub fn create_directory_object(
        path: impl AsRef<Path>,
        mode: Option<Mode>,
        mtime: Option<u64>,
        ctime: Option<u64>,
    ) -> Self {
        Self::new(path, mode, mtime, ctime, None, None, None)
    }

    pub fn create_symlink_object(
        from_path: impl AsRef<Path>,
        to_path: impl AsRef<Path>,
        mode: Option<Mode>,
        mtime: Option<u64>,
        ctime: Option<u64>,
    ) -> Self {
        Self::new(
            to_path,
            mode,
            mtime,
            ctime,
            None,
            None,
            Some(FilerefData::Text(
                from_path.as_ref().to_string_lossy().into_owned(),
            )),
        )
    }

    pub fn create_empty_file_object(
        path: impl AsRef<Path>,
        mode: Option<Mode>,
        mtime: Option<u64>,
        ctime: Option<u64>,
    ) -> Self {
        Self::new(path, mode, mtime, ctime, Some(0), None, None)
    }

    pub fn create_json_content_object(
        path: impl AsRef<Path>,
        data: Value,
        mode: Option<Mode>,
        mtime: Option<u64>,
        ctime: Option<u64>,
    ) -> Self {
        Self::new(
            path,
            mode,
            mtime,
            ctime,
            None,
            None,
            Some(FilerefData::Json(data)),
        )
    }

    pub fn create_text_content_object<D>(
        path: impl AsRef<Path>,
        data: D,
        mode: Option<Mode>,
        mtime: Option<u64>,
        ctime: Option<u64>,
    ) -> Self
    where
        D: ToString,
    {
        let data_str = data.to_string();
        Self::new(
            path,
            mode,
            mtime,
            ctime,
            Some(data_str.len()),
            Some(FilerefEncoding::Utf8),
            Some(FilerefData::Text(data_str)),
        )
    }

    pub fn create_literal_binary_object<I>(
        path: impl AsRef<Path>,
        data: I,
        mode: Option<Mode>,
        mtime: Option<u64>,
        ctime: Option<u64>,
    ) -> Self
    where
        I: IntoIterator<Item = u8>,
    {
        let data_vec: Vec<u8> = data.into_iter().collect();
        Self::new(
            path,
            mode,
            mtime,
            ctime,
            Some(data_vec.len()),
            Some(FilerefEncoding::Base64),
            Some(FilerefData::Encoded(data_vec)),
        )
    }

    pub fn create_from_text_file(
        fileref_path: impl AsRef<Path>,
        fs_path: impl AsRef<Path>,
    ) -> Result<Self> {
        let data = std::fs::read_to_string(fs_path.as_ref())?;
        let stat = lstat(fs_path.as_ref())?;
        Ok(Self::create_text_content_object(
            fileref_path,
            data,
            Some(Mode::from_bits_truncate(stat.st_mode)),
            Some(stat.st_mtime as _),
            Some(stat.st_ctime as _),
        ))
    }

    pub fn create_from_binary_file(
        fileref_path: impl AsRef<Path>,
        fs_path: impl AsRef<Path>,
    ) -> Result<Self> {
        let data = std::fs::read(fs_path.as_ref())?;
        let stat = lstat(fs_path.as_ref())?;
        Ok(Self::create_literal_binary_object(
            fileref_path,
            data,
            Some(Mode::from_bits_truncate(stat.st_mode)),
            Some(stat.st_mtime as _),
            Some(stat.st_ctime as _),
        ))
    }

    pub fn create_from_file(
        fileref_path: impl AsRef<Path>,
        fs_path: impl AsRef<Path>,
    ) -> Result<Self> {
        match Self::create_from_text_file(fileref_path.as_ref(), fs_path.as_ref()) {
            Ok(fileref) => Ok(fileref),
            Err(e) => match e {
                FluxError::System(_, ref sys_err) | FluxError::Io(ref sys_err) => {
                    if let ErrorKind::InvalidData = sys_err.kind() {
                        Self::create_from_binary_file(fileref_path.as_ref(), fs_path.as_ref())
                    } else {
                        Err(e)
                    }
                }
                _ => Err(e),
            },
        }
    }
}
