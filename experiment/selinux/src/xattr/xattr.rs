
pub fn set_xattr(fpath: &str, attr: &str, data: &[u8], flags: i64) -> Result<(), std::io::Error> {
    unimplemented!("not implemented yet")
}

pub fn lset_xattr(fpath: &str, attr: &str, data: &[u8], flags: i64) -> Result<(), std::io::Error> {
    unimplemented!("not implemented yet")
}

pub fn get_xattr(fpath: &str, attr: &str) -> Result<String, std::io::Error> {
    unimplemented!("not implemented yet")
}

pub fn lget_xattr(fpath: &str, attr: &str) -> Result<String, std::io::Error> {
    unimplemented!("not implemented yet")
}