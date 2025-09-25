use clap::Parser;
use regex::Regex;
use std::collections::HashSet;
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "collect-unmarked-tests")]
#[command(about = "Collect Python tests that don't have specific markers")]
struct Args {
    /// Test directory to scan
    #[arg(default_value = "tests")]
    test_dir: PathBuf,

    /// Markers to exclude (default: unit,integration,component,skip,slow)
    #[arg(long, value_delimiter = ',')]
    exclude_markers: Option<Vec<String>>,

    /// Whitelisted package modules to scan (for monorepo support)
    #[arg(long, value_delimiter = ',')]
    packages: Option<Vec<String>>,
}

fn main() {
    let args = Args::parse();

    let default_markers = vec![
        "unit".to_string(),
        "integration".to_string(),
        "component".to_string(),
        "skip".to_string(),
        "slow".to_string(),
    ];

    let exclude_markers: HashSet<String> = args
        .exclude_markers
        .unwrap_or(default_markers)
        .into_iter()
        .collect();

    let unmarked_tests = if let Some(packages) = &args.packages {
        collect_unmarked_tests_for_packages(packages, &exclude_markers)
    } else {
        collect_unmarked_tests(&args.test_dir, &exclude_markers)
    };

    if unmarked_tests.is_empty() {
        println!("No unmarked tests found.");
    } else {
        eprintln!("Found {} unmarked test(s):", unmarked_tests.len());
        for test in &unmarked_tests {
            eprintln!("  {}", test);
        }
        std::process::exit(1);
    }
}

fn collect_unmarked_tests_for_packages(
    packages: &[String],
    exclude_markers: &HashSet<String>,
) -> Vec<String> {
    let mut unmarked_tests = Vec::new();

    for package in packages {
        let package_dir = PathBuf::from(package);
        if package_dir.exists() {
            unmarked_tests.extend(collect_unmarked_tests(&package_dir, exclude_markers));
        }
    }

    unmarked_tests
}

fn collect_unmarked_tests(test_dir: &PathBuf, exclude_markers: &HashSet<String>) -> Vec<String> {
    let mut unmarked_tests = Vec::new();

    for entry in WalkDir::new(test_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "py"))
    {
        if let Ok(content) = std::fs::read_to_string(entry.path()) {
            let tests = find_python_test_functions(&content, exclude_markers);
            for test in tests {
                unmarked_tests.push(format!("{}::{}", entry.path().display(), test));
            }
        }
    }

    unmarked_tests
}

fn find_python_test_functions(content: &str, exclude_markers: &HashSet<String>) -> Vec<String> {
    let mut test_functions = Vec::new();

    // Regex to match test functions (allow whitespace at start)
    let test_fn_regex = Regex::new(r"^(\s*)def\s+(test_\w+)\s*\(").unwrap();
    // Regex to match class definitions
    let class_regex = Regex::new(r"^(\s*)class\s+(\w+)").unwrap();

    let lines: Vec<&str> = content.lines().collect();

    // Track class-level markers
    let mut class_markers: Vec<(usize, HashSet<String>)> = Vec::new(); // (indent_level, markers)

    for (i, line) in lines.iter().enumerate() {
        // Check for class definitions and their markers
        if let Some(captures) = class_regex.captures(line) {
            let class_indent = captures.get(1).unwrap().as_str().len();
            let mut class_level_markers = HashSet::new();

            // Look backwards for class-level decorators
            let mut j = i;
            let mut brace_depth = 0;
            let mut paren_depth = 0;
            let mut bracket_depth = 0;

            while j > 0 {
                j -= 1;
                let prev_line = lines[j];
                let trimmed = prev_line.trim();

                if trimmed.is_empty() {
                    continue;
                }

                // Count braces, parentheses, and brackets
                for ch in trimmed.chars() {
                    match ch {
                        '(' => paren_depth += 1,
                        ')' => paren_depth -= 1,
                        '[' => bracket_depth += 1,
                        ']' => bracket_depth -= 1,
                        '{' => brace_depth += 1,
                        '}' => brace_depth -= 1,
                        _ => {}
                    }
                }

                if trimmed.starts_with('@') {
                    if let Some(marker) = extract_pytest_marker(trimmed) {
                        class_level_markers.insert(marker);
                    }
                    if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 {
                        // Continue to look for more decorators
                    }
                } else if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 {
                    break;
                }
            }

            // Remove any previous class markers at same or deeper indentation
            class_markers.retain(|(indent, _)| *indent < class_indent);

            // Add this class's markers if any
            if !class_level_markers.is_empty() {
                class_markers.push((class_indent, class_level_markers));
            }
            continue;
        }

        if let Some(captures) = test_fn_regex.captures(line) {
            let function_name = captures.get(2).unwrap().as_str();
            let function_indent = captures.get(1).unwrap().as_str().len();

            // Check if this function is in a class with excluded markers
            let mut has_excluded_marker = false;
            for (class_indent, markers) in &class_markers {
                if function_indent > *class_indent {
                    // This function is inside this class
                    for marker in markers {
                        if exclude_markers.contains(marker) {
                            has_excluded_marker = true;
                            break;
                        }
                    }
                    if has_excluded_marker {
                        break;
                    }
                }
            }

            // If not marked by class, check function-level decorators
            if !has_excluded_marker {
                // Start from the line before the function and work backwards
                let mut j = i;
                let mut brace_depth = 0;
                let mut paren_depth = 0;
                let mut bracket_depth = 0;

                while j > 0 {
                    j -= 1;
                    let line = lines[j];
                    let trimmed = line.trim();

                    // Skip blank lines
                    if trimmed.is_empty() {
                        continue;
                    }

                    // Count braces, parentheses, and brackets to handle multi-line decorators
                    for ch in trimmed.chars() {
                        match ch {
                            '(' => paren_depth += 1,
                            ')' => paren_depth -= 1,
                            '[' => bracket_depth += 1,
                            ']' => bracket_depth -= 1,
                            '{' => brace_depth += 1,
                            '}' => brace_depth -= 1,
                            _ => {}
                        }
                    }

                    // If the line starts with @, it's a decorator
                    if trimmed.starts_with('@') {
                        if let Some(marker) = extract_pytest_marker(trimmed)
                            && exclude_markers.contains(&marker)
                        {
                            has_excluded_marker = true;
                            break;
                        }
                        // If we're at balanced braces/parens/brackets, this decorator is complete
                        if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 {
                            // Continue to look for more decorators
                        }
                    } else if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 {
                        // We're not in a multi-line decorator and this isn't a decorator line
                        // This means we've gone past all decorators for this function
                        break;
                    }
                    // Otherwise, this is part of a multi-line decorator, keep going
                }
            }

            if !has_excluded_marker {
                test_functions.push(function_name.to_string());
            }
        }
    }

    test_functions
}

fn extract_pytest_marker(decorator_line: &str) -> Option<String> {
    // Handle various pytest marker formats:
    // @pytest.mark.unit
    // @pytest.mark.parametrize(...)
    // @pytest.mark.skip

    let marker_regex = Regex::new(r"@(?:pytest\.mark\.)?(\w+)").unwrap();

    marker_regex
        .captures(decorator_line)
        .map(|captures| captures.get(1).unwrap().as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_pytest_marker() {
        assert_eq!(
            extract_pytest_marker("@pytest.mark.unit"),
            Some("unit".to_string())
        );
        assert_eq!(
            extract_pytest_marker("@pytest.mark.slow"),
            Some("slow".to_string())
        );
        assert_eq!(extract_pytest_marker("@unit"), Some("unit".to_string()));
        assert_eq!(extract_pytest_marker("@skip"), Some("skip".to_string()));
        assert_eq!(
            extract_pytest_marker("@pytest.mark.parametrize('x', [1, 2])"),
            Some("parametrize".to_string())
        );
    }

    #[test]
    fn test_find_python_test_functions() {
        let content = r#"
import pytest

@pytest.mark.unit
def test_marked_function():
    pass

def test_unmarked_function():
    pass

@pytest.mark.skip
def test_skipped_function():
    pass

def test_another_unmarked():
    pass
"#;

        let exclude_markers: HashSet<String> =
            ["unit", "skip"].iter().map(|s| s.to_string()).collect();
        let result = find_python_test_functions(content, &exclude_markers);

        assert_eq!(
            result,
            vec!["test_unmarked_function", "test_another_unmarked"]
        );
    }

    #[test]
    fn test_multiline_decorator() {
        let content = r#"
import pytest

@pytest.mark.unit
@pytest.mark.parametrize(
    "arg1, arg2",
    [
        pytest.param("a", "b"),
        pytest.param("c", "d"),
    ],
)
def test_with_multiline_decorator():
    pass

def test_unmarked():
    pass
"#;

        let exclude_markers: HashSet<String> = ["unit"].iter().map(|s| s.to_string()).collect();
        let result = find_python_test_functions(content, &exclude_markers);

        assert_eq!(result, vec!["test_unmarked"]);
    }

    #[test]
    fn test_class_methods() {
        let content = r#"
import pytest

class TestExample:
    @pytest.mark.unit
    def test_marked_method(self):
        pass

    def test_unmarked_method(self):
        pass

    @pytest.mark.integration
    def test_another_marked_method(self):
        pass

def test_function_level():
    pass

class TestAnother:
    def test_unmarked_in_class(self):
        pass
"#;

        let exclude_markers: HashSet<String> = ["unit", "integration"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let result = find_python_test_functions(content, &exclude_markers);

        assert_eq!(
            result,
            vec![
                "test_unmarked_method",
                "test_function_level",
                "test_unmarked_in_class"
            ]
        );
    }

    #[test]
    fn test_class_level_markers() {
        let content = r#"
import pytest

@pytest.mark.unit
class TestMarkedClass:
    def test_method_in_marked_class(self):
        pass

    @pytest.mark.integration
    def test_method_with_own_marker(self):
        pass

class TestUnmarkedClass:
    def test_method_in_unmarked_class(self):
        pass

def test_function_level():
    pass
"#;

        let exclude_markers: HashSet<String> = ["unit", "integration"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let result = find_python_test_functions(content, &exclude_markers);

        assert_eq!(
            result,
            vec!["test_method_in_unmarked_class", "test_function_level"]
        );
    }
}
