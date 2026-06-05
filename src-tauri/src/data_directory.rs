use std::{
    io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataDirectoryMode {
    OsAppData,
    PortableExecutable,
}

pub fn default_data_directory_mode() -> DataDirectoryMode {
    if cfg!(all(windows, not(debug_assertions))) {
        DataDirectoryMode::PortableExecutable
    } else {
        DataDirectoryMode::OsAppData
    }
}

pub fn runtime_data_dir(app_data_dir: PathBuf) -> io::Result<PathBuf> {
    match default_data_directory_mode() {
        DataDirectoryMode::OsAppData => Ok(app_data_dir),
        DataDirectoryMode::PortableExecutable => {
            portable_data_dir_for_exe(std::env::current_exe()?)
        }
    }
}

pub fn portable_data_dir_for_exe(exe_path: impl AsRef<Path>) -> io::Result<PathBuf> {
    let exe_path = exe_path.as_ref();
    let exe_dir = exe_path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "portable executable path has no parent directory",
        )
    })?;
    if exe_dir.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "portable executable path has no parent directory",
        ));
    }

    Ok(exe_dir.join("data"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_data_dir_sits_next_to_executable() {
        let data_dir = portable_data_dir_for_exe(Path::new("C:/Apps/Brawler/brawler.exe"))
            .expect("portable data path should resolve");

        assert_eq!(data_dir, PathBuf::from("C:/Apps/Brawler/data"));
    }

    #[test]
    fn rejects_executable_path_without_parent() {
        let error = portable_data_dir_for_exe(Path::new("brawler.exe"))
            .expect_err("relative bare executable path has no parent");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}
