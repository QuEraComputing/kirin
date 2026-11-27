//! The generated program: `size` functions `@f0` .. `@f{size-1}` and a
//! `@leaf`, all in the `lowered` stage. See README.md.

/// How the functions call each other.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    /// Every `@fi` calls `@leaf`.
    Independent,
    /// `@fi` calls `@f{i+1}`; the last calls `@leaf`.
    CallChain,
}

impl Shape {
    pub fn parse(name: &str) -> Self {
        match name {
            "independent" => Self::Independent,
            "call_chain" => Self::CallChain,
            _ => panic!("unknown shape {name}"),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Independent => "independent",
            Self::CallChain => "call_chain",
        }
    }

    /// The functions whose stage-wide analysis root reaches `@f{edited}`.
    pub fn affected(self, edited: usize) -> Vec<String> {
        match self {
            Self::Independent => vec![format!("f{edited}")],
            Self::CallChain => (0..=edited).map(|i| format!("f{i}")).collect(),
        }
    }
}

/// Which functions hold work for the passes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// Every function.
    Cold,
    /// Only `@f0`.
    EditFirst,
    /// Only the last `@fi`.
    EditLast,
}

impl Case {
    pub fn parse(name: &str) -> Self {
        match name {
            "cold" => Self::Cold,
            "edit_first" => Self::EditFirst,
            "edit_last" => Self::EditLast,
            _ => panic!("unknown case {name}"),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::EditFirst => "edit_first",
            Self::EditLast => "edit_last",
        }
    }

    /// The one edited function, if only one holds work.
    pub fn edited(self, size: usize) -> Option<usize> {
        match self {
            Self::Cold => None,
            Self::EditFirst => Some(0),
            Self::EditLast => Some(size - 1),
        }
    }

    pub fn is_dirty(self, size: usize, i: usize) -> bool {
        self.edited(size).is_none_or(|edited| edited == i)
    }
}

/// The program text. Each `@fi(%x, %q)` never reads `%q` and holds `units`
/// three-statement units. A dirty function's units compute constants the
/// fold pass replaces, and it passes `%d = mul %x, %x` as its callee's
/// unread `%q`, which the free pass replaces. A clean function has as many
/// statements, none of them foldable, and passes `%x`.
pub fn text(shape: Shape, case: Case, size: usize, units: usize) -> String {
    let mut text = String::new();
    for i in 0..size {
        text.push_str(&format!("stage @lowered fn @f{i}(i64, i64) -> i64;\n"));
    }
    text.push_str("stage @lowered fn @leaf(i64, i64) -> i64;\n");
    for i in 0..size {
        let dirty = case.is_dirty(size, i);
        text.push_str(&format!(
            "specialize @lowered fn @f{i}(i64, i64) -> i64 cfg {{\n  ^entry(%x: i64, %q: i64) {{\n"
        ));
        let mut acc = "x".to_string();
        for j in 0..units {
            if dirty {
                text.push_str(&format!(
                    "    %a{j} = constant {} -> i64;\n    %b{j} = add %a{j}, %a{j} -> i64;\n",
                    j + 1
                ));
            } else {
                text.push_str(&format!(
                    "    %a{j} = sub %x, %x -> i64;\n    %b{j} = add %a{j}, %x -> i64;\n"
                ));
            }
            text.push_str(&format!("    %acc{j} = add %{acc}, %b{j} -> i64;\n"));
            acc = format!("acc{j}");
        }
        let unread = if dirty {
            text.push_str("    %d = mul %x, %x -> i64;\n");
            "d"
        } else {
            "x"
        };
        let callee = match shape {
            Shape::CallChain if i + 1 < size => format!("f{}", i + 1),
            _ => "leaf".to_string(),
        };
        text.push_str(&format!(
            "    %r = call.named @{callee}(%x, %{unread}) -> i64;\n    %out = add %r, %{acc} -> i64;\n    ret %out;\n  }}\n}}\n"
        ));
    }
    text.push_str(
        "specialize @lowered fn @leaf(i64, i64) -> i64 cfg {\n  ^entry(%p: i64, %q: i64) {\n    ret %p;\n  }\n}\n",
    );
    text
}
