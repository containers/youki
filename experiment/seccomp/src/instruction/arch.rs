use crate::instruction::Instruction;
use crate::instruction::*;

pub enum Arch {
    X86,
}

impl Arch {
    pub fn gen_validate(&self) -> Vec<Instruction> {
        let arch = match self {
            Arch::X86 => AUDIT_ARCH_X86_64,
        };

        vec![
            Instruction::stmt(BPF_LD | BPF_W | BPF_ABS, SECCOMP_DATA_ARCH_OFFSET as u32),
            Instruction::jump(BPF_JMP | BPF_JEQ | BPF_K, arch, 1, 0),
            Instruction::stmt(BPF_RET | BPF_K, SECCOMP_RET_KILL_PROCESS),
        ]
    }
}
