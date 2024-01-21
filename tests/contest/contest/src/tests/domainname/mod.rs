use crate::utils::test_inside_container;
use oci_spec::runtime::{ProcessBuilder, Spec, SpecBuilder};
use test_framework::{ConditionalTest, TestGroup, TestResult};

fn get_spec(domainname: &str) -> Spec {
    SpecBuilder::default()
        .domainname(domainname)
        .process(
            ProcessBuilder::default()
                .args(vec![
                    "runtimetest".to_string(),
                    "domainname_test".to_string(),
                ])
                .build()
                .expect("error in creating process config"),
        )
        .build()
        .unwrap()
}

fn set_domainname_test() -> TestResult {
    let spec = get_spec("domainname");
    test_inside_container(spec, &|_| Ok(()))
}

pub fn get_domainname_tests() -> TestGroup {
    let mut tg = TestGroup::new("domainname_test");
    let set_domainname_test = ConditionalTest::new(
        "set_domainname_test",
        Box::new(|| match std::env::var("RUNTIME_KIND") {
            Err(_) => true,
            Ok(s) => s != "runc",
        }),
        Box::new(set_domainname_test),
    );
    tg.add(vec![Box::new(set_domainname_test)]);

    tg
}
