use kirin_derive::Dialect;
use kirin_ir::*;
use kirin_test_utils::*;

fn val(i: usize) -> SSAValue {
    TestSSAValue(i).into()
}

fn res_val(i: usize) -> ResultValue {
    TestSSAValue(i).into()
}

#[test]
fn test_dialect_properties() {
    #[derive(Dialect, Clone, Debug, PartialEq)]
    #[kirin(fn, type_lattice = SimpleTypeLattice, crate = kirin_ir)]
    enum MyLang {
        #[kirin(pure)]
        Add(SSAValue, SSAValue, ResultValue),
        #[kirin(terminator)]
        Return(SSAValue),
        #[kirin(constant)]
        Const(i64, ResultValue),
        Other(SSAValue),
    }

    let v1 = val(1);
    let v2 = val(2);
    let res = res_val(3);

    let add = MyLang::Add(v1, v2, res);
    assert!(add.is_pure());
    assert!(!add.is_terminator());
    assert!(!add.is_constant());

    let ret = MyLang::Return(v1);
    assert!(!ret.is_pure());
    assert!(ret.is_terminator());
    assert!(!ret.is_constant());

    let c = MyLang::Const(42, res);
    assert!(!c.is_pure());
    assert!(!c.is_terminator());
    assert!(c.is_constant());

    let other = MyLang::Other(v1);
    assert!(!other.is_pure());
    assert!(!other.is_terminator());
    assert!(!other.is_constant());
}

#[test]
fn test_struct_field_iterators() {
    #[derive(Dialect, Clone, Debug, PartialEq)]
    #[kirin(fn, type_lattice = SimpleTypeLattice, crate = kirin_ir)]
    struct MyOp {
        arg1: SSAValue,
        arg2: SSAValue,
        extra: String,
        res: ResultValue,
    }

    let v1 = val(1);
    let v2 = val(2);
    let r1 = res_val(3);

    let op = MyOp {
        arg1: v1,
        arg2: v2,
        extra: "hello".to_string(),
        res: r1,
    };

    let args: Vec<_> = op.arguments().cloned().collect();
    assert_eq!(args, vec![v1, v2]);

    let results: Vec<_> = op.results().cloned().collect();
    assert_eq!(results, vec![r1]);
}

#[test]
fn test_enum_field_iterators() {
    #[derive(Dialect, Clone, Debug, PartialEq)]
    #[kirin(fn, type_lattice = SimpleTypeLattice, crate = kirin_ir)]
    enum MyEnum {
        One { arg: SSAValue },
        Two(SSAValue, SSAValue),
        Three,
    }

    let v1 = val(1);
    let v2 = val(2);

    let one = MyEnum::One { arg: v1 };
    assert_eq!(one.arguments().count(), 1);
    assert_eq!(one.arguments().next(), Some(&v1));

    let two = MyEnum::Two(v1, v2);
    let args: Vec<_> = two.arguments().cloned().collect();
    assert_eq!(args, vec![v1, v2]);

    let three = MyEnum::Three;
    assert_eq!(three.arguments().count(), 0);
}

#[test]
fn test_vec_fields() {
    // Removed 'fn' to disable builder generation, avoiding 'ResultValue field cannot be a Vec' error
    #[derive(Dialect, Clone, Debug, PartialEq)]
    #[kirin(type_lattice = SimpleTypeLattice, crate = kirin_ir)]
    struct VecOp {
        args: Vec<SSAValue>,
        res: Vec<ResultValue>,
    }

    let v1 = val(1);
    let v2 = val(2);
    let r1 = res_val(3);

    let op = VecOp {
        args: vec![v1, v2],
        res: vec![r1],
    };

    let args: Vec<_> = op.arguments().cloned().collect();
    assert_eq!(args, vec![v1, v2]);

    let results: Vec<_> = op.results().cloned().collect();
    assert_eq!(results, vec![r1]);
}
