use std::{
    io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataDirectoryMode {
    OsAppData,
    HomeDotBrawler,
    PortableExecutable,
}

pub fn default_data_directory_mode() -> DataDirectoryMode {
    if cfg!(all(windows, not(debug_assertions))) {
        DataDirectoryMode::PortableExecutable
    } else if cfg!(all(target_os = "linux", not(debug_assertions))) {
        DataDirectoryMode::HomeDotBrawler
    } else {
        DataDirectoryMode::OsAppData
    }
}

pub fn runtime_data_dir(app_data_dir: PathBuf) -> io::Result<PathBuf> {
    match default_data_directory_mode() {
        DataDirectoryMode::OsAppData => Ok(app_data_dir),
        DataDirectoryMode::HomeDotBrawler => linux_home_data_dir(),
        DataDirectoryMode::PortableExecutable => {
            portable_data_dir_for_exe(std::env::current_exe()?)
        }
    }
}

fn linux_home_data_dir() -> io::Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "HOME is required to resolve the Linux Brawler data directory",
        )
    })?;

    linux_home_data_dir_from_home(PathBuf::from(home))
}

fn linux_home_data_dir_from_home(home: PathBuf) -> io::Result<PathBuf> {
    if home.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "HOME is required to resolve the Linux Brawler data directory",
        ));
    }

    Ok(home.join(".brawler"))
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

    #[test]
    fn linux_home_data_dir_uses_dot_brawler_under_home() {
        let data_dir = linux_home_data_dir_from_home(PathBuf::from("/home/example"))
            .expect("home data path should resolve");

        assert_eq!(data_dir, PathBuf::from("/home/example/.brawler"));
    }
}
