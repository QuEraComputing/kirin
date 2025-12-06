#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct Config {
    /// Number of spaces to use for each indentation level.
    pub tab_spaces: usize,
    /// Maximum width of each line.
    pub max_width: usize,
    /// Whether to include line numbers in the output.
    pub line_numbers: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tab_spaces: 4,
            max_width: 120,
            line_numbers: true,
        }
    }
}

impl Config {
    pub fn with_width(mut self, width: usize) -> Self {
        self.max_width = width;
        self
    }

    pub fn with_tab_spaces(mut self, spaces: usize) -> Self {
        self.tab_spaces = spaces;
        self
    }

    pub fn with_line_numbers(mut self, line_numbers: bool) -> Self {
        self.line_numbers = line_numbers;
        self
    }
}
