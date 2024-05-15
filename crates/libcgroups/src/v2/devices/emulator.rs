use oci_spec::runtime::{LinuxDeviceCgroup, LinuxDeviceType};

// For cgroup v1 compatibility, runc implements a device emulator to calculate the final rules given
// a list of user-defined rules.
// https://github.com/opencontainers/runc/commit/2353ffec2bb670a200009dc7a54a56b93145f141
//
// I chose to implement a very simple algorithm, which will just work in most cases, but with
// diversion from cgroupv1 in some cases:
//  1. just add used-defined rules one by one
//  2. discard existing rules when encountering a rule with type='a', and change to deny/allow all
//     list according the 'allow' of the rule
//  3. bpf program will check rule one by one in *reversed* order, return action of first rule
//     which matches device access operation
//

// FIXME: should we use runc's implementation?
pub struct Emulator {
    pub default_allow: bool,
    pub rules: Vec<LinuxDeviceCgroup>,
}

impl Emulator {
    pub fn with_default_allow(default_allow: bool) -> Self {
        Emulator {
            default_allow,
            rules: Vec::new(),
        }
    }

    pub fn add_rules(&mut self, rules: &[LinuxDeviceCgroup]) {
        for rule in rules {
            self.add_rule(rule);
        }
    }

    pub fn add_rule(&mut self, rule: &LinuxDeviceCgroup) {
        // special case, switch to blacklist or whitelist and clear all existing rules
        // NOTE: we ignore other fields when type='a', this is same as cgroup v1 and runc
        if rule.typ().unwrap_or_default() == LinuxDeviceType::A {
            self.default_allow = rule.allow();
            self.rules.clear();
            return;
        }

        // empty access match nothing, just discard this rule
        if rule.access().is_none() {
            return;
        }

        self.rules.push(rule.clone());
    }
}

#[cfg(test)]
mod tests {
    use oci_spec::runtime::LinuxDeviceCgroupBuilder;

    use super::*;

    #[test]
    fn test_with_default_allow() {
        // act
        let emulator = Emulator::with_default_allow(true);

        // assert
        assert_eq!(emulator.rules.len(), 0);
        assert!(emulator.default_allow);
    }

    #[test]
    fn test_type_a_rule() {
        // arrange
        let mut emulator = Emulator::with_default_allow(false);
        let cgroup = LinuxDeviceCgroupBuilder::default()
            .typ(LinuxDeviceType::A)
            .build()
            .unwrap();

        // act
        emulator.add_rule(&cgroup);

        // assert
        assert_eq!(emulator.rules.len(), 0);
        assert!(!emulator.default_allow);
    }

    #[test]
    fn test_add_empty_rule() {
        // arrange
        let mut emulator = Emulator::with_default_allow(false);
        let cgroup = LinuxDeviceCgroupBuilder::default().build().unwrap();

        // act
        emulator.add_rule(&cgroup);

        // assert
        assert_eq!(emulator.rules.len(), 0);
        assert!(!emulator.default_allow);
    }

    #[test]
    fn test_add_some_rule() {
        // arrange
        let mut emulator = Emulator::with_default_allow(false);
        let permission: &str = "PERMISSION";
        let cgroup = LinuxDeviceCgroupBuilder::default()
            .typ(LinuxDeviceType::B)
            .access(permission)
            .build()
            .unwrap();

        // act
        emulator.add_rule(&cgroup);

        // assert
        let top_rule = emulator.rules.first().unwrap();
        assert_eq!(top_rule.access(), &Some(permission.to_string()));
        assert!(!emulator.default_allow);
    }
}
