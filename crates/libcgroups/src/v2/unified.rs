use std::{collections::HashMap, path::Path};

use super::controller_type::ControllerType;
use crate::common::{self, ControllerOpt, WrappedIoError};

#[derive(thiserror::Error, Debug)]
pub enum V2UnifiedError {
    #[error("io error: {0}")]
    WrappedIo(#[from] WrappedIoError),
    #[error("subsystem {subsystem} is not available: {err}")]
    SubsystemNotAvailable {
        subsystem: String,
        err: WrappedIoError,
    },
}

pub struct Unified {}

impl Unified {
    pub fn apply(
        controller_opt: &ControllerOpt,
        cgroup_path: &Path,
        controllers: Vec<ControllerType>,
    ) -> Result<(), V2UnifiedError> {
        if let Some(unified) = &controller_opt.resources.unified() {
            Self::apply_impl(unified, cgroup_path, &controllers)?;
        }

        Ok(())
    }

    fn apply_impl(
        unified: &HashMap<String, String>,
        cgroup_path: &Path,
        controllers: &[ControllerType],
    ) -> Result<(), V2UnifiedError> {
        log::debug!("Apply unified cgroup config");
        for (cgroup_file, value) in unified {
            if let Err(err) = common::write_cgroup_file_str(cgroup_path.join(cgroup_file), value) {
                let (subsystem, _) = cgroup_file.split_once('.').unwrap_or((&cgroup_file, ""));

                if controllers.iter().any(|c| c.to_string() == subsystem) {
                    Err(err)?;
                } else {
                    return Err(V2UnifiedError::SubsystemNotAvailable {
                        subsystem: subsystem.into(),
                        err,
                    });
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;

    use oci_spec::runtime::LinuxResourcesBuilder;

    use crate::test::{create_temp_dir, set_fixture};
    use crate::v2::controller_type::ControllerType;

    use super::*;

    #[test]
    fn test_set_unified() {
        // arrange
        let tmp = create_temp_dir("test_set_unified").unwrap();
        let hugetlb_limit_path = set_fixture(&tmp, "hugetlb.1GB.limit_in_bytes", "").unwrap();
        let cpu_weight_path = set_fixture(&tmp, "cpu.weight", "").unwrap();

        let unified = {
            let mut u = HashMap::new();
            u.insert(
                "hugetlb.1GB.limit_in_bytes".to_owned(),
                "72348034".to_owned(),
            );
            u.insert("cpu.weight".to_owned(), "5000".to_owned());
            u
        };

        let resources = LinuxResourcesBuilder::default()
            .unified(unified)
            .build()
            .unwrap();

        let controller_opt = ControllerOpt {
            resources: &resources,
            freezer_state: None,
            oom_score_adj: None,
            disable_oom_killer: false,
        };

        // act
        Unified::apply(&controller_opt, &tmp, vec![]).expect("apply unified");

        // assert
        let hugetlb_limit = fs::read_to_string(hugetlb_limit_path).expect("read hugetlb limit");
        let cpu_weight = fs::read_to_string(cpu_weight_path).expect("read cpu weight");
        assert_eq!(hugetlb_limit, "72348034");
        assert_eq!(cpu_weight, "5000");
    }

    #[test]
    fn test_set_unified_failed_to_write_subsystem_not_enabled() {
        // arrange
        let tmp =
            create_temp_dir("test_set_unified_failed_to_write_subsystem_not_enabled").unwrap();

        let unified = {
            let mut u = HashMap::new();
            u.insert(
                "hugetlb.1GB.limit_in_bytes".to_owned(),
                "72348034".to_owned(),
            );
            u.insert("cpu.weight".to_owned(), "5000".to_owned());
            u
        };

        let resources = LinuxResourcesBuilder::default()
            .unified(unified)
            .build()
            .unwrap();

        let controller_opt = ControllerOpt {
            resources: &resources,
            freezer_state: None,
            oom_score_adj: None,
            disable_oom_killer: false,
        };

        // act
        let result = Unified::apply(&controller_opt, &tmp, vec![]);

        // assert
        assert!(result.is_err());
    }

    #[test]
    fn test_set_unified_failed_to_write_subsystem_enabled() {
        // arrange
        let tmp = create_temp_dir("test_set_unified_failed_to_write_subsystem_enabled").unwrap();

        let unified = {
            let mut u = HashMap::new();
            u.insert(
                "hugetlb.1GB.limit_in_bytes".to_owned(),
                "72348034".to_owned(),
            );
            u.insert("cpu.weight".to_owned(), "5000".to_owned());
            u
        };

        let resources = LinuxResourcesBuilder::default()
            .unified(unified)
            .build()
            .unwrap();

        let controller_opt = ControllerOpt {
            resources: &resources,
            oom_score_adj: None,
            disable_oom_killer: false,
            freezer_state: None,
        };

        // act
        let result = Unified::apply(
            &controller_opt,
            &tmp,
            vec![ControllerType::HugeTlb, ControllerType::Cpu],
        );

        // assert
        assert!(result.is_err());
    }
}
