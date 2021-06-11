use super::structs::*;
use crate::*;

#[cglue_trait]
pub trait GenWithSelfConstraint: Send {
    fn gwsc_1(&self, input: &usize) -> bool;
}

impl GenWithSelfConstraint for SA {
    fn gwsc_1(&self, input: &usize) -> bool {
        *input == 55
    }
}

#[test]
fn use_self_constraint() {
    let sa = SA {};

    let obj = trait_obj!(sa as GenWithSelfConstraint);

    let ret = std::thread::spawn(move || obj.gwsc_1(&55)).join().unwrap();

    assert!(ret);
}
