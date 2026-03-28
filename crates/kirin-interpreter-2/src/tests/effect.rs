use crate::{
    ConsumeEffect, FromConstant, Lift, Machine,
    control::Shell,
    effect::{Cursor, Stateless},
};

#[test]
fn cursor_lifts_to_shell() {
    let advance: Shell<i64, ()> = Cursor::<()>::Advance.lift();
    assert_eq!(advance, Shell::Advance);

    let stay: Shell<i64, ()> = Cursor::<()>::Stay.lift();
    assert_eq!(stay, Shell::Stay);

    let jump: Shell<i64, i32> = Cursor::Jump(42_i32).lift();
    assert_eq!(jump, Shell::Replace(42));
}

#[test]
fn stateless_machine_consumes_cursor_effects() {
    let mut machine = Stateless::<i64>::default();

    let advance = machine.consume_effect(Cursor::Advance).unwrap();
    let stay = machine.consume_effect(Cursor::Stay).unwrap();

    assert_eq!(advance, Shell::Advance);
    assert_eq!(stay, Shell::Stay);
}

#[test]
fn stateless_machine_implements_machine_contract() {
    fn effect_roundtrip<'ir, M>(_: &M)
    where
        M: Machine<'ir, Effect = Cursor, Stop = i64, Seed = ()>,
    {
    }

    let machine = Stateless::<i64>::default();
    effect_roundtrip(&machine);
}

#[test]
fn cursor_variants_are_distinct() {
    let advance = Cursor::<()>::Advance;
    let stay = Cursor::<()>::Stay;
    assert_ne!(advance, stay);
}

#[test]
fn lift_identity_returns_self() {
    let cursor: Cursor<()> = Cursor::Advance;
    let lifted: Cursor<()> = cursor.lift();
    assert_eq!(lifted, Cursor::Advance);
}

#[test]
fn from_constant_converts_via_try_from() {
    // i64 implements From<i32>, so TryFrom<i32> is satisfied
    let value: i64 = FromConstant::from_constant(42_i32).unwrap();
    assert_eq!(value, 42_i64);
}
