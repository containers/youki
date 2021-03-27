mod child;
pub mod fork;
mod init;
pub mod message;
mod parent;

pub enum Process {
    Parent(parent::ParentProcess),
    Child(child::ChildProcess),
    Init(init::InitProcess),
}
