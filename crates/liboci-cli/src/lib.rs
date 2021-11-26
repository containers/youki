use std::fmt::Debug;

use clap::Parser;

// Subcommands that are specified in https://github.com/opencontainers/runtime-tools/blob/master/docs/command-line-interface.md

mod create;
mod delete;
mod kill;
mod start;
mod state;

pub use {create::Create, delete::Delete, kill::Kill, start::Start, state::State};

// Other common subcommands that aren't specified in the document
mod events;
mod exec;
mod list;
mod pause;
mod ps;
mod resume;
mod run;
mod spec;

pub use {
    events::Events, exec::Exec, list::List, pause::Pause, ps::Ps, resume::Resume, run::Run,
    spec::Spec,
};

// Subcommands parsed by liboci-cli, based on the [OCI
// runtime-spec](https://github.com/opencontainers/runtime-spec/blob/master/runtime.md)
// and specifically the [OCI Command Line
// Interface](https://github.com/opencontainers/runtime-tools/blob/master/docs/command-line-interface.md)
#[derive(Parser, Debug)]
pub enum StandardCmd {
    Create(Create),
    Start(Start),
    State(State),
    Kill(Kill),
    Delete(Delete),
}

// Extra subcommands not documented in the OCI Command Line Interface,
// but found in
// [runc](https://github.com/opencontainers/runc/blob/master/man/runc.8.md)
// and other runtimes.
#[derive(Parser, Debug)]
pub enum CommonCmd {
    Events(Events),
    Exec(Exec),
    List(List),
    Pause(Pause),
    #[clap(setting=clap::AppSettings::AllowLeadingHyphen)]
    Ps(Ps),
    Resume(Resume),
    Run(Run),
    Spec(Spec),
}
