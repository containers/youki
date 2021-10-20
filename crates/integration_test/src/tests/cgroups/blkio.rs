// use anyhow::{Context, Result};
// use oci_spec::runtime::{
//     LinuxBlockIo, LinuxBlockIoBuilder, LinuxBuilder, LinuxResourcesBuilder,
//     LinuxThrottleDeviceBuilder, LinuxWeightDeviceBuilder, Spec, SpecBuilder,
// };
// use test_framework::{ConditionalTest, TestGroup, TestResult};

// fn create_spec() -> Result<Spec> {
//     let major = 8;
//     let minor = 0;

//     let spec = SpecBuilder::default()
//         .linux(
//             LinuxBuilder::default()
//                 .resources(
//                     LinuxResourcesBuilder::default()
//                         .block_io(
//                             LinuxBlockIoBuilder::default()
//                                 .weight(500u16)
//                                 .leaf_weight(100u16)
//                                 .weight_device(vec![
//                                     LinuxWeightDeviceBuilder::default()
//                                         .major(major)
//                                         .minor(minor)
//                                         .weight(200u16)
//                                         .build()
//                                         .context("could not build weight device")?,
//                                     // LinuxWeightDeviceBuilder::default()
//                                     //     .major(8)
//                                     //     .minor(1)
//                                     //     .weight(300u16)
//                                     //     .build()
//                                     //     .context("could not build weight device")?,
//                                 ])
//                                 .throttle_read_bps_device(vec![
//                                     LinuxThrottleDeviceBuilder::default()
//                                         .major(major)
//                                         .minor(minor)
//                                         .rate(value)
//                                         .build()
//                                         .context("could not build read bps device spec")?,
//                                 ])
//                                 .throttle_read_iops_device(vec![
//                                     LinuxThrottleDeviceBuilder::default()
//                                         .major(major)
//                                         .minor(minor)
//                                         .build()
//                                         .context("could not build read iops device spec")?,
//                                 ])
//                                 .throttle_write_bps_device(vec![
//                                     LinuxThrottleDeviceBuilder::default()
//                                         .major(major)
//                                         .minor(minor)
//                                         .build()
//                                         .context("could not build write bps device spec")?,
//                                 ])
//                                 .throttle_write_iops_device(vec![
//                                     LinuxThrottleDeviceBuilder::default()
//                                         .major(major)
//                                         .minor(minor)
//                                         .build()
//                                         .context("could not build write iops device spec")?,
//                                 ])
//                                 .build()
//                                 .context("could not build block io spec")?,
//                         )
//                         .build()
//                         .context("could not build resource spec")?,
//                 )
//                 .build()
//                 .context("could not build linux spec")?,
//         )
//         .build()
//         .context("could not build spec")?;

//     Ok(spec)
// }

// pub fn get_test_group<'a>() -> TestGroup<'a> {
//     let mut test_group = TestGroup::new("cgroup_blkio");
//     let test = ConditionalTest::new("first", Box::new(validate), Box::new(test_blkio));

//     test_group.add(vec![Box::new(test)]);
//     test_group
// }

// pub fn test_blkio() -> TestResult {
//     todo!();
// }

// pub fn validate() -> bool {
//     todo!();
// }
