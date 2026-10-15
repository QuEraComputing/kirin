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
        "stage @test fn @benchmark(i64, i64, i64) -> i64;\nspecialize @test fn @benchmark(i64, i64, i64) -> i64 cfg {\n^entry(%self: i64, %x: i64, %cond: i64) {\n",
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
        "nested_if" => {
            text.push_str("%outer = if %cond then block ^outer_then() {\n%inner_a = if %cond then block ^a_then() {\n");
            let a = chain(&mut text, size / 4, "a", "x", "x");
            text.push_str(&format!("yield %{a};\n}} else block ^a_else() {{\n"));
            let b = chain(&mut text, size / 4, "b", "x", "x");
            text.push_str(&format!("yield %{b};\n}} -> i64;\nyield %inner_a;\n}} else block ^outer_else() {{\n%inner_b = if %cond then block ^b_then() {{\n"));
            let c = chain(&mut text, size / 4, "c", "x", "x");
            text.push_str(&format!("yield %{c};\n}} else block ^b_else() {{\n"));
            let d = chain(&mut text, size / 4, "d", "x", "x");
            text.push_str(&format!(
                "yield %{d};\n}} -> i64;\nyield %inner_b;\n}} -> i64;\nret %outer;\n}}\n"
            ));
        }
        _ => panic!("unknown family"),
    }
    text.push_str("}\n");
    text
}
