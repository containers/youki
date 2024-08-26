use crate::instruction::Instruction;
use crate::instruction::*;

#[derive(PartialEq)]
pub enum Arch {
    X86,AArch64
}

pub fn gen_validate(arc: &Arch) -> Vec<Instruction> {
    let arch = match arc {
        Arch::X86 => AUDIT_ARCH_X86_64,
        Arch::AArch64 => AUDIT_ARCH_AARCH64
    };

    vec![
        Instruction::stmt(BPF_LD | BPF_W | BPF_ABS, seccomp_data_arch_offset() as u32),
        Instruction::jump(BPF_JMP | BPF_JEQ | BPF_K, 1, 0, arch),
        Instruction::stmt(BPF_RET | BPF_K, SECCOMP_RET_KILL_PROCESS),
    ]
}
