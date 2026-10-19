//! Rust-specific text builder for the shared semantic specifications.
use super::spec::{Family, Path, Suite};

fn chain(text: &mut String, count: usize, path: &Path, prefix: &str, x: &str, y: &str) -> String {
    let operand = |name: &str| match name {
        "x" => x,
        "y" => y,
        _ => panic!("unknown input"),
    };
    let mut previous = operand(&path.start).to_string();
    let delta = operand(&path.delta);
    for i in 0..count {
        let next = format!("{prefix}{i}");
        text.push_str(&format!(
            "%{next} = {} %{previous}, %{delta} -> i64;\n",
            path.operation
        ));
        previous = next;
    }
    previous
}

fn leaf(text: &mut String, family: &Family, path_name: &str, size: usize, merge: &str) {
    let x = format!("{path_name}x");
    let y = format!("{path_name}y");
    text.push_str(&format!("^{path_name}(%{x}: i64, %{y}: i64) {{\n"));
    let result = chain(
        text,
        size / family.steps_divisor,
        family.path(path_name),
        path_name,
        &x,
        &y,
    );
    text.push_str(&format!("br ^{merge}(%{result});\n}}\n"));
}

pub fn workload(name: &str, size: usize) -> String {
    let suite = Suite::load();
    let family = suite.family(name, size);
    // Rust needs no Python Method self parameter. Only semantic inputs are fed.
    let mut text = String::from(
        "stage @test fn @benchmark(i64, i64, bool, bool) -> i64;\nspecialize @test fn @benchmark(i64, i64, bool, bool) -> i64 cfg {\n^entry(%x: i64, %y: i64, %outer: bool, %inner: bool) {\n",
    );
    match name {
        "chain" => {
            let result = chain(&mut text, size, family.path("all"), "a", "x", "y");
            text.push_str(&format!("ret %{result};\n}}\n"));
        }
        "diamond" => {
            text.push_str("cond_br %outer then=^left(%x, %y) else=^right(%x, %y);\n}\n");
            leaf(&mut text, family, "left", size, "merge");
            leaf(&mut text, family, "right", size, "merge");
            text.push_str("^merge(%answer: i64) { ret %answer; }\n");
        }
        "nested_cfg" => {
            text.push_str("cond_br %outer then=^left(%x, %y) else=^right(%x, %y);\n}\n");
            text.push_str("^left(%lx: i64, %ly: i64) { cond_br %inner then=^a(%lx, %ly) else=^b(%lx, %ly); }\n");
            text.push_str("^right(%rx: i64, %ry: i64) { cond_br %inner then=^c(%rx, %ry) else=^d(%rx, %ry); }\n");
            for (path, merge) in [
                ("a", "left_merge"),
                ("b", "left_merge"),
                ("c", "right_merge"),
                ("d", "right_merge"),
            ] {
                leaf(&mut text, family, path, size, merge);
            }
            text.push_str("^left_merge(%lm: i64) { br ^merge(%lm); }\n^right_merge(%rm: i64) { br ^merge(%rm); }\n^merge(%answer: i64) { ret %answer; }\n");
        }
        _ => panic!("unknown family"),
    }
    text.push_str("}\n");
    text
}
