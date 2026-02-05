//! Configuration for pretty printing.

/// Configuration options for pretty printing.
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
            tab_spaces: 2,
            max_width: 120,
            line_numbers: true,
        }
    }
}

impl Config {
    /// Create a new config with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum line width.
    pub fn with_width(mut self, width: usize) -> Self {
        self.max_width = width;
        self
    }

    /// Set the number of spaces per indentation level.
    pub fn with_tab_spaces(mut self, spaces: usize) -> Self {
        self.tab_spaces = spaces;
        self
    }

    /// Set whether to include line numbers.
    pub fn with_line_numbers(mut self, line_numbers: bool) -> Self {
        self.line_numbers = line_numbers;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.tab_spaces, 2);
        assert_eq!(config.max_width, 120);
        assert!(config.line_numbers);
    }

    #[test]
    fn test_config_builder() {
        let config = Config::new()
            .with_width(80)
            .with_tab_spaces(4)
            .with_line_numbers(false);

        assert_eq!(config.max_width, 80);
        assert_eq!(config.tab_spaces, 4);
        assert!(!config.line_numbers);
    }

    #[test]
    fn test_config_clone() {
        let config1 = Config::new().with_width(100);
        let config2 = config1.clone();
        assert_eq!(config1, config2);
    }
}
