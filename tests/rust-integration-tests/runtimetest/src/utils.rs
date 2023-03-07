use nix::sys::stat::stat;
use nix::sys::stat::SFlag;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn test_file_read_access(path: &str) -> Result<(), std::io::Error> {
    let _ = std::fs::OpenOptions::new()
        .create(false)
        .read(true)
        .open(path)?;
    Ok(())
}

fn test_dir_read_access(path: &str) -> Result<(), std::io::Error> {
    let _ = std::fs::read_dir(path)?;
    Ok(())
}

fn is_file_like(mode: u32) -> bool {
    // for this please refer
    // https://stackoverflow.com/questions/40163270/what-is-s-isreg-and-what-does-it-do
    // https://linux.die.net/man/2/stat
    mode & SFlag::S_IFREG.bits() != 0 || mode & SFlag::S_IFCHR.bits() != 0
}

fn is_dir(mode: u32) -> bool {
    mode & SFlag::S_IFDIR.bits() != 0
}

pub fn test_read_access(path: &str) -> Result<(), std::io::Error> {
    let fstat = stat(path)?;
    let mode = fstat.st_mode;
    if is_file_like(mode) {
        // we have a file or a char/block device
        return test_file_read_access(path);
    } else if is_dir(mode) {
        return test_dir_read_access(path);
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        format!("cannot test read access for {path:?}, has mode {mode:x}"),
    ))
}

fn test_file_write_access(path: &str) -> Result<(), std::io::Error> {
    let _ = std::fs::OpenOptions::new().write(true).open(path)?;
    Ok(())
}

fn test_dir_write_access(path: &str) -> Result<(), std::io::Error> {
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(PathBuf::from(path).join("test.txt"))?;
    Ok(())
}

pub fn test_write_access(path: &str) -> Result<(), std::io::Error> {
    let fstat = stat(path)?;
    let mode = fstat.st_mode;
    if is_file_like(mode) {
        // we have a file or a char/block device
        return test_file_write_access(path);
    } else if is_dir(mode) {
        return test_dir_write_access(path);
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        format!("cannot test write access for {path:?}, has mode {mode:x}"),
    ))
}

pub fn test_file_executable(path: &str) -> Result<(), std::io::Error> {
    let fstat = stat(path)?;
    let mode = fstat.st_mode;
    if is_file_like(mode) {
        Command::new(path).output()?;
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        format!("{path:?} is directory, so cannot execute"),
    ))
}

pub fn test_dir_update_access_time(path: &str) -> Result<(), std::io::Error> {
    println!("test_dir_update_access_time path: {:?}", path);
    let metadata = fs::metadata(PathBuf::from(path))?;
    let rest = metadata.accessed();
    let first_access_time = rest.unwrap();
    println!(
        "{:?} dir first access time is {:?}",
        path, first_access_time
    );
    // execute ls command to update access time
    Command::new("ls")
        .arg(path)
        .output()
        .expect("execute ls command error");
    // second get access time
    let metadata = fs::metadata(PathBuf::from(path))?;
    let rest = metadata.accessed();
    let second_access_time = rest.unwrap();
    println!(
        "{:?} dir second access time is {:?}",
        path, second_access_time
    );
    if first_access_time == second_access_time {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("cannot update access time for path {:?}", path),
        ));
    }
    Ok(())
}

pub fn test_dir_not_update_access_time(path: &str) -> Result<(), std::io::Error> {
    println!("test_dir_not_update_access_time path: {:?}", path);
    let metadata = fs::metadata(PathBuf::from(path))?;
    let rest = metadata.accessed();
    let first_access_time = rest.unwrap();
    println!(
        "{:?} dir first access time is {:?}",
        path, first_access_time
    );
    // execute ls command to update access time
    Command::new("ls")
        .arg(path)
        .output()
        .expect("execute ls command error");
    // second get access time
    let metadata = fs::metadata(PathBuf::from(path))?;
    let rest = metadata.accessed();
    let second_access_time = rest.unwrap();
    println!(
        "{:?} dir second access time is {:?}",
        path, second_access_time
    );
    if first_access_time != second_access_time {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("cannot update access time for path {:?}", path),
        ));
    }
    Ok(())
}

pub fn test_device_access(path: &str) -> Result<(), std::io::Error> {
    println!("test_device_access path: {:?}", path);
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(PathBuf::from(path).join("null"))?;
    Ok(())
}

pub fn test_device_unaccess(path: &str) -> Result<(), std::io::Error> {
    println!("test_device_unaccess path: {:?}", path);
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(PathBuf::from(path).join("null"))?;
    Ok(())
}
