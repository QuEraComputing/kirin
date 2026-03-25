use crate::{
    ConsumeEffect, Machine,
    control::Shell,
    effect::{Flow, Stateless},
};

#[test]
fn flow_maps_advance_and_stop_to_shell_controls() {
    assert_eq!(Flow::<i64>::Advance.into_shell(), Shell::Advance);
    assert_eq!(Flow::Stop(7).into_shell(), Shell::Stop(7));
}

#[test]
fn stateless_machine_consumes_shared_flow_effects() {
    let mut machine = Stateless::<i64>::default();

    let advance = machine.consume_effect(Flow::Advance).unwrap();
    let stop = machine.consume_effect(Flow::Stop(9)).unwrap();

    assert_eq!(advance, Shell::Advance);
    assert_eq!(stop, Shell::Stop(9));
}

#[test]
fn stateless_machine_implements_machine_contract() {
    fn effect_roundtrip<'ir, M>(_: &M)
    where
        M: Machine<'ir, Effect = Flow<i64>, Stop = i64>,
    {
    }

    let machine = Stateless::<i64>::default();
    effect_roundtrip(&machine);
}
