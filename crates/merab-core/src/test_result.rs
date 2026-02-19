use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRunResult {
    pub passed: u32,
    pub failed: u32,
    pub output: String,
    pub success: bool,
}

impl TestRunResult {
    pub fn new(output: String) -> Self {
        let output_lower = output.to_lowercase();

        let passed = extract_test_count(&output_lower, "passed");
        let failed = extract_test_count(&output_lower, "failed");

        let success = passed > 0 && failed == 0;

        Self {
            passed,
            failed,
            output,
            success,
        }
    }

    pub fn no_tests_found(output: String) -> Self {
        Self {
            passed: 0,
            failed: 0,
            output,
            success: false,
        }
    }
}

fn extract_test_count(output: &str, kind: &str) -> u32 {
    for line in output.lines() {
        if line.contains(kind) {
            if let Some(num) = line.split_whitespace().find_map(|w| w.parse::<u32>().ok()) {
                return num;
            }
        }
    }

    if output.contains("ok.") || output.contains("PASSED") || output.contains("passed") {
        if kind == "passed" {
            return 1;
        }
    }
    if output.contains("FAILED") || output.contains("FAIL") || output.contains("failed") {
        if kind == "failed" {
            return 1;
        }
    }

    0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestRunner {
    Cargo,
    Npm,
    Pytest,
    GoTest,
    Unknown,
}

impl TestRunner {
    pub fn detect(project_path: &std::path::Path) -> Self {
        if project_path.join("Cargo.toml").exists() {
            return TestRunner::Cargo;
        }
        if project_path.join("package.json").exists() {
            return TestRunner::Npm;
        }
        if project_path.join("pytest.ini").exists()
            || project_path.join("pyproject.toml").exists()
            || project_path.join("setup.py").exists()
            || project_path.join("requirements.txt").exists()
        {
            return TestRunner::Pytest;
        }
        if project_path.join("go.mod").exists() {
            return TestRunner::GoTest;
        }
        TestRunner::Unknown
    }

    pub fn command(&self) -> (&'static str, &'static [&'static str]) {
        match self {
            TestRunner::Cargo => ("cargo", &["test", "--", "--nocapture"]),
            TestRunner::Npm => ("npm", &["test", "--", "--if-present"]),
            TestRunner::Pytest => ("pytest", &["-v", "--tb=short"]),
            TestRunner::GoTest => ("go", &["test", "./...", "-v"]),
            TestRunner::Unknown => ("true", &[]),
        }
    }
}
