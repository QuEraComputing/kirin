//! Rust text for the rewrite shapes. `chain`, `diamond` and `nested_cfg` are
//! the shared builders; the others are local to this benchmark and mirrored
//! by `../workload.py`.
use super::common::workload as shared;

const HEADER: &str = "stage @test fn @benchmark(i64, i64, bool, bool) -> i64;
specialize @test fn @benchmark(i64, i64, bool, bool) -> i64 cfg {
^entry(%x: i64, %y: i64, %outer: bool, %inner: bool) {
";

/// `@benchmark` holding `size` statements built by `line(i, previous)`, then
/// returning `ret`, or the last result if `ret` is `None`.
fn straight_line(size: usize, ret: Option<&str>, line: impl Fn(usize, &str) -> String) -> String {
    let mut text = HEADER.to_string();
    let mut previous = "x".to_string();
    for i in 0..size {
        text.push_str(&line(i, &previous));
        previous = format!("s{i}");
    }
    text.push_str(&format!("ret %{};\n}}\n}}\n", ret.unwrap_or(&previous)));
    text
}

pub fn text(shape: &str, size: usize) -> String {
    match shape {
        "chain" | "diamond" | "nested_cfg" => shared::workload(shape, size),
        // -(-(...(x))) with `size` negations; `size` is even, so it is `x`.
        "neg_chain" => straight_line(size, None, |i, previous| {
            format!("%s{i} = neg %{previous} -> i64;\n")
        }),
        // `size` unused `x + y`.
        "dead_independent" => straight_line(size, Some("x"), |i, _| {
            format!("%s{i} = add %x, %y -> i64;\n")
        }),
        // x + y + ... + y, `size` additions, unused.
        "dead_chain" => straight_line(size, Some("x"), |i, previous| {
            format!("%s{i} = add %{previous}, %y -> i64;\n")
        }),
        // `folds_F_match_M`: `size` additions, then `2F` negations.
        _ if shape.starts_with("folds_") => {
            let folds: usize = shape.split('_').nth(1).unwrap().parse().unwrap();
            straight_line(size + 2 * folds, None, |i, previous| {
                if i < size {
                    format!("%s{i} = add %{previous}, %y -> i64;\n")
                } else {
                    format!("%s{i} = neg %{previous} -> i64;\n")
                }
            })
        }
        // Each subtraction of k expands into k subtractions of one.
        _ if shape.starts_with("unroll_") => {
            let k: usize = shape["unroll_".len()..].parse().expect("unroll_<k>");
            straight_line(size / 2, None, |i, previous| {
                format!("%k{i} = constant {k} -> i64;\n%s{i} = sub %{previous}, %k{i} -> i64;\n")
            })
        }
        _ => panic!("unknown shape {shape}"),
    }
}
