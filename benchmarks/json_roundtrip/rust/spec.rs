use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
pub struct Suite {
    pub max_size: usize,
    pub max_abs_input: i64,
    pub samples: Vec<Input>,
    pub families: Vec<Family>,
}
#[derive(Deserialize, Clone)]
pub struct Input {
    pub x: i64,
    pub y: i64,
    pub outer: bool,
    pub inner: bool,
}
#[derive(Deserialize)]
pub struct Family {
    pub name: String,
    pub steps_divisor: usize,
    pub paths: Vec<Path>,
}
#[derive(Deserialize)]
pub struct Path {
    pub name: String,
    pub when: BTreeMap<String, bool>,
    pub start: String,
    pub delta: String,
    pub operation: String,
    #[serde(default)]
    pub trips: usize,
    pub expected: Expected,
}
#[derive(Deserialize)]
pub struct Expected {
    pub x_multiplier: Coefficient,
    pub y_multiplier: Coefficient,
}
#[derive(Deserialize)]
pub struct Coefficient {
    pub constant: i64,
    pub per_step: i64,
}
impl Suite {
    pub fn load() -> Self {
        serde_json::from_str(include_str!("../workloads.json")).unwrap()
    }
    pub fn family(&self, name: &str, size: usize) -> &Family {
        assert!(size > 0 && size <= self.max_size && size.is_multiple_of(4));
        self.families
            .iter()
            .find(|f| f.name == name)
            .expect("known semantic workload")
    }
}
impl Family {
    pub fn path(&self, name: &str) -> &Path {
        self.paths.iter().find(|p| p.name == name).unwrap()
    }
    pub fn expected(&self, size: usize, input: &Input) -> (&str, i64) {
        let paths: Vec<_> = self
            .paths
            .iter()
            .filter(|p| {
                p.when.iter().all(|(k, v)| {
                    *v == match k.as_str() {
                        "outer" => input.outer,
                        "inner" => input.inner,
                        _ => panic!("unknown input"),
                    }
                })
            })
            .collect();
        assert_eq!(paths.len(), 1, "exactly one oracle path must match");
        let path = paths[0];
        let steps = i64::try_from(size / self.steps_divisor).unwrap();
        let coefficient = |v: &Coefficient| {
            v.per_step
                .checked_mul(steps)
                .unwrap()
                .checked_add(v.constant)
                .unwrap()
        };
        let result = coefficient(&path.expected.x_multiplier)
            .checked_mul(input.x)
            .unwrap()
            .checked_add(
                coefficient(&path.expected.y_multiplier)
                    .checked_mul(input.y)
                    .unwrap(),
            )
            .unwrap();
        (&path.name, result)
    }
}
