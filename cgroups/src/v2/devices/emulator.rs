use anyhow::Result;
use oci_spec::*;

// For cgroup v1 compatiblity, runc implements a device emulator to caculate the final rules given
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

    pub fn add_rules(&mut self, rules: &Vec<oci_spec::LinuxDeviceCgroup>) -> Result<()> {
        for rule in rules {
            self.add_rule(rule)?;
        }
        Ok(())
    }

    pub fn add_rule(&mut self, rule: &oci_spec::LinuxDeviceCgroup) -> Result<()> {
        // special case, switch to blacklist or whitelist and clear all existing rules
        // NOTE: we ignore other fields when type='a', this is same as cgroup v1 and runc
        if rule.typ.clone().unwrap_or_default() == oci_spec::LinuxDeviceType::A {
            self.default_allow = rule.allow;
            self.rules.clear();
            return Ok(());
        }

        // empty access match nothing, just discard this rule
        if rule.access.is_none() {
            return Ok(());
        }

        self.rules.push(rule.clone());
        Ok(())
    }
}

// FIXME: add some tests
