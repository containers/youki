use anyhow::Result;
use libcontainer::workload::default::DefaultExecutor;
use youki::youki_main;

fn main() -> Result<()> {
    youki_main(DefaultExecutor {})
}
