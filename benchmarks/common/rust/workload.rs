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

/// The function called by the call families, emitted before `@benchmark`.
fn callee(name: &str, path: &Path) -> String {
    let op = &path.operation;
    match name {
        "calls" | "calls_same" => format!(
            "stage @test fn @step(i64, i64) -> i64;\nspecialize @test fn @step(i64, i64) -> i64 cfg {{\n^entry(%a: i64, %b: i64) {{ %r = {op} %a, %b -> i64; ret %r; }}\n}}\n"
        ),
        "recursion" => format!(
            "stage @test fn @rec(i64, i64, i64) -> i64;\nspecialize @test fn @rec(i64, i64, i64) -> i64 cfg {{\n^entry(%n: i64, %acc: i64, %d: i64) {{\n%zero = constant 0 -> i64;\n%done = le %n, %zero -> bool;\ncond_br %done then=^base() else=^step();\n}}\n^base() {{ ret %acc; }}\n^step() {{\n%one = constant 1 -> i64;\n%m = sub %n, %one -> i64;\n%next = {op} %acc, %d -> i64;\n%r = call.named @rec(%m, %next, %d) -> i64;\nret %r;\n}}\n}}\n"
        ),
        _ => String::new(),
    }
}

pub fn workload(name: &str, size: usize) -> String {
    let suite = Suite::load();
    let family = suite.family(name, size);
    let mut text = callee(name, &family.paths[0]);
    // Rust needs no Python Method self parameter. Only semantic inputs are fed.
    text.push_str(
        "stage @test fn @benchmark(i64, i64, bool, bool) -> i64;\nspecialize @test fn @benchmark(i64, i64, bool, bool) -> i64 cfg {\n^entry(%x: i64, %y: i64, %outer: bool, %inner: bool) {\n",
    );
    let steps = size / family.steps_divisor;
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
        "loop" => {
            let (run, skip) = (family.path("run"), family.path("skip"));
            text.push_str(&format!(
                "%zero = constant 0 -> i64;\n%run = constant {} -> i64;\n%skip = constant {} -> i64;\ncond_br %outer then=^head(%zero, %x, %run) else=^head(%zero, %x, %skip);\n}}\n",
                run.trips, skip.trips
            ));
            text.push_str("^head(%i: i64, %acc: i64, %n: i64) {\n%more = lt %i, %n -> bool;\ncond_br %more then=^body() else=^exit();\n}\n^body() {\n");
            let result = chain(&mut text, steps, run, "b", "acc", "y");
            text.push_str(&format!(
                "%one = constant 1 -> i64;\n%next = add %i, %one -> i64;\nbr ^head(%next, %{result}, %n);\n}}\n^exit() {{ ret %acc; }}\n"
            ));
        }
        "calls" | "calls_same" => {
            let path = family.path("all");
            let (start, delta) = (format!("%{}", path.start), format!("%{}", path.delta));
            let mut previous = start.clone();
            for i in 0..steps {
                text.push_str(&if name == "calls" {
                    format!("%c{i} = call.named @step({previous}, {delta}) -> i64;\n")
                } else {
                    format!("%s{i} = call.named @step({start}, {delta}) -> i64;\n%c{i} = add {previous}, %s{i} -> i64;\n")
                });
                previous = format!("%c{i}");
            }
            text.push_str(&format!("ret {previous};\n}}\n"));
        }
        "recursion" => {
            let path = family.path("run");
            text.push_str(&format!(
                "%run = constant {steps} -> i64;\n%skip = constant 0 -> i64;\ncond_br %outer then=^go(%run) else=^go(%skip);\n}}\n^go(%depth: i64) {{\n%r = call.named @rec(%depth, %{}, %{}) -> i64;\nret %r;\n}}\n",
                path.start, path.delta
            ));
        }
        _ => panic!("unknown family"),
    }
    text.push_str("}\n");
    text
}
