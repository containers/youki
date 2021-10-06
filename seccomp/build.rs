const MINIMUM_VERSION: &str = "2.5";
const PKG_NAME: &str = "libseccomp";
fn main() {
    match pkg_config::Config::new()
        .atleast_version(MINIMUM_VERSION)
        .probe(PKG_NAME)
    {
        Ok(_) => return,
        Err(err) => {
            eprintln!(
                "{:?} could not be found meeting minimum version requirement {:?}: {}",
                PKG_NAME, MINIMUM_VERSION, err
            );
            std::process::exit(1);
        }
    }
}
