
// function compatible with setxattr in golang.org/x/sys/unix repo.
// set_xattr sets extended attributes on a file specified by its path.
pub fn set_xattr(fpath: &str, attr: &str, data: &[u8], flags: i64) -> Result<(), std::io::Error> {
    unimplemented!("not implemented yet")
}

// function compatible with lsetxattr in golang.org/x/sys/unix repo.
// lset_xattr sets extended attributes on a symbolic link.
pub fn lset_xattr(fpath: &str, attr: &str, data: &[u8], flags: i64) -> Result<(), std::io::Error> {
    unimplemented!("not implemented yet")
}

// function compatible with getattr in go-selinux repo.
// get_xattr returns the value of an extended attribute attr set for path.
pub fn get_xattr(fpath: &str, attr: &str) -> Result<String, std::io::Error> {
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

// function compatible with lgetxattr in go-selinux repo.
// lget_xattr returns the value of an extended attribute attr set for path.
pub fn lget_xattr(fpath: &str, attr: &str) -> Result<String, std::io::Error> {
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
