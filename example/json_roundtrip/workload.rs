fn chain(text: &mut String, count: usize, prefix: &str, input: &str, other: &str) -> String {
    let mut previous = input.to_string();
    for i in 0..count {
        let next = format!("{prefix}{i}");
        text.push_str(&format!("%{next} = add %{previous}, %{other} -> i64;\n"));
        previous = next;
    }
    previous
}

pub fn workload(family: &str, size: usize) -> String {
    let mut text = String::from(
        "stage @test fn @benchmark(i64, i64, bool) -> i64;\nspecialize @test fn @benchmark(i64, i64, bool) -> i64 cfg {\n^entry(%self: i64, %x: i64, %cond: bool) {\n",
    );
    match family {
        "chain" => {
            let result = chain(&mut text, size, "a", "x", "x");
            text.push_str(&format!("ret %{result};\n}}\n"));
        }
        "diamond" => {
            text.push_str("cond_br %cond then=^left(%x) else=^right(%x);\n}\n^left(%l: i64) {\n");
            let left = chain(&mut text, size / 2, "l", "l", "l");
            text.push_str(&format!("br ^merge(%{left});\n}}\n^right(%r: i64) {{\n"));
            let right = chain(&mut text, size / 2, "r", "r", "r");
            text.push_str(&format!(
                "br ^merge(%{right});\n}}\n^merge(%answer: i64) {{ ret %answer; }}\n"
            ));
        }
        "nested_cfg" => {
            text.push_str("cond_br %cond then=^left(%x) else=^right(%x);\n}\n");
            text.push_str("^left(%l: i64) { cond_br %cond then=^a(%l) else=^b(%l); }\n");
            text.push_str("^right(%r: i64) { cond_br %cond then=^c(%r) else=^d(%r); }\n");
            for (prefix, merge) in [
                ("a", "left_merge"),
                ("b", "left_merge"),
                ("c", "right_merge"),
                ("d", "right_merge"),
            ] {
                text.push_str(&format!("^{prefix}(%{prefix}: i64) {{\n"));
                let result = chain(&mut text, size / 4, prefix, prefix, prefix);
                text.push_str(&format!("br ^{merge}(%{result});\n}}\n"));
            }
            text.push_str("^left_merge(%lm: i64) { br ^merge(%lm); }\n^right_merge(%rm: i64) { br ^merge(%rm); }\n^merge(%answer: i64) { ret %answer; }\n");
        }
        _ => panic!("unknown family"),
    }
    text.push_str("}\n");
    text
}
