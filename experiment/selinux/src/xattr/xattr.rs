use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum XattrError {
    #[error("Failed to set_xattr: {0}")]
    SetXattr(String),
    #[error("Failed to lset_xattr: {0}")]
    LSetXattr(String),
    #[error("Failed to get_xattr: {0}")]
    GetXattr(String),
    #[error("Failed to call lget_xattr: {0}")]
    LGetXattr(String),    
}

// function similar with setxattr in golang.org/x/sys/unix repo.
// set_xattr sets extended attributes on a file specified by its path.
pub fn set_xattr(fpath: &Path, attr: &str, data: &[u8], flags: i64) -> Result<(), XattrError> {
    unimplemented!("not implemented yet")
}

// function similar with lsetxattr in golang.org/x/sys/unix repo.
// lset_xattr sets extended attributes on a symbolic link.
pub fn lset_xattr(fpath: &Path, attr: &str, data: &[u8], flags: i64) -> Result<(), XattrError> {
    unimplemented!("not implemented yet")
}

// function similar with getattr in go-selinux repo.
// get_xattr returns the value of an extended attribute attr set for path.
pub fn get_xattr(fpath: &Path, attr: &str) -> Result<String, XattrError> {
    unimplemented!("not implemented yet")
    /*
    match label {
        Ok(mut v) => {            
            if (!v.is_empty()) && (v.chars().last() == Some('\x00')) {
                v = (&v[0..v.len() - 1]).to_string();
            }
            return Ok(v);
        },
        Err(e) => return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("get_xattr failed: {}", e),
        ))
    }    
    */
}

// function similar with lgetxattr in go-selinux repo.
// lget_xattr returns the value of an extended attribute attr set for path.
pub fn lget_xattr(fpath: &Path, attr: &str) -> Result<String, XattrError> {
    unimplemented!("not implemented yet")
    /*
    match label {
        Ok(mut v) => {            
            if (!v.is_empty()) && (v.chars().last() == Some('\x00')) {
                v = (&v[0..v.len() - 1]).to_string();
            }
            return Ok(v);
        },
        Err(e) => return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("lget_xattr failed: {}", e),
        ))
    }    
     */
}
