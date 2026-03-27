use crate::{
    ConsumeEffect, FromConstant, Lift, Machine,
    control::Shell,
    effect::{Cursor, Flow, Stateless},
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

#[test]
fn cursor_variants_are_distinct() {
    let advance = Cursor::Advance;
    let stay = Cursor::Stay;
    assert_ne!(advance, stay);
}

#[test]
fn lift_identity_returns_self() {
    let flow: Flow<i64> = Flow::Advance;
    let lifted: Flow<i64> = flow.lift();
    assert_eq!(lifted, Flow::Advance);
}

#[test]
fn from_constant_converts_via_try_from() {
    // i64 implements From<i32>, so TryFrom<i32> is satisfied
    let value: i64 = FromConstant::from_constant(42_i32).unwrap();
    assert_eq!(value, 42_i64);
}
