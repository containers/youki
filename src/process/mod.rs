pub mod fork;
pub mod message;

mod child;
mod init;
mod parent;

pub use init::InitProcess;

pub enum Process {
    Parent(parent::ParentProcess),
    Child(child::ChildProcess),
    Init(init::InitProcess),
}
