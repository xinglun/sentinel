mod confirmation;
mod episodes;
mod net_value;
mod runner;
mod utility;

#[cfg(test)]
pub(super) use confirmation::build_confirmation_cost;
#[cfg(test)]
pub(super) use episodes::build_validation_report;
#[cfg(test)]
pub(super) use net_value::build_net_decision_value;
pub use runner::run_core_simulation;
#[cfg(test)]
pub(super) use runner::{raw_top_candidates, retain_active_lifecycle_entries};

#[cfg(test)]
mod tests {
    include!("simulation/simulation_tests.rs");
}
