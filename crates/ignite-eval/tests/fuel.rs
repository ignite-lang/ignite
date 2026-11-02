//! The limits of `docs/slice.md` section 15.3 (O14).

use ignite_eval::Fuel;

#[test]
fn default_limits_are_one_million() {
    let fuel = Fuel::default();
    assert_eq!(fuel.units, 1_000_000);
    assert_eq!(fuel.nodes, 1_000_000);
}

#[test]
fn with_units_keeps_the_node_limit() {
    let fuel = Fuel::with_units(10);
    assert_eq!(fuel.units, 10);
    assert_eq!(fuel.nodes, 1_000_000);
}
