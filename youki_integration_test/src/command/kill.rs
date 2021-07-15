use std::path::PathBuf;
use std::process::{Command, Stdio};

// TODO Allow to receive arguments.
// TODO Wrapping up the results
pub fn exec(project_path: &PathBuf, id: &str) -> bool {
    let status = Command::new(project_path.join(PathBuf::from("youki")))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .arg("-r")
        .arg(project_path.join("integration-workspace").join("youki"))
        .arg("kill")
        .arg(id)
        .arg("9")
        .status()
        .expect("failed to execute process");
    return status.success();
}
