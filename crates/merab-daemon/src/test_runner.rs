use std::path::Path;
use std::process::Command;

use merab_core::{TestRunResult, TestRunner};

pub async fn run_tests(project_path: &Path) -> TestRunResult {
    let runner = TestRunner::detect(project_path);
    
    if runner == TestRunner::Unknown {
        return TestRunResult::no_tests_found(
            "No test runner detected (Cargo.toml, package.json, pytest.ini, go.mod)".to_string()
        );
    }
    
    let (cmd, args) = runner.command();
    let path_str = project_path.to_string_lossy().to_string();
    
    let output = tokio::task::spawn_blocking(move || {
        let mut cmd = Command::new(cmd);
        cmd.args(args)
           .current_dir(&path_str)
           .output()
    }).await;
    
    match output {
        Ok(Ok(result)) => {
            let stdout = String::from_utf8_lossy(&result.stdout).to_string();
            let stderr = String::from_utf8_lossy(&result.stderr).to_string();
            let combined = if stderr.is_empty() { stdout } else { format!("{}\n{}", stdout, stderr) };
            
            TestRunResult::new(combined)
        }
        Ok(Err(e)) => {
            TestRunResult::no_tests_found(format!("Failed to run tests: {}", e))
        }
        Err(e) => {
            TestRunResult::no_tests_found(format!("Task join error: {}", e))
        }
    }
}

pub fn detect_test_runner(project_path: &std::path::Path) -> TestRunner {
    TestRunner::detect(project_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_cargo_project() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "[package]").unwrap();
        let runner = TestRunner::detect(tmp.path());
        assert!(matches!(runner, TestRunner::Cargo));
    }

    #[test]
    fn test_detect_npm_project() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("package.json"), "{}").unwrap();
        let runner = TestRunner::detect(tmp.path());
        assert!(matches!(runner, TestRunner::Npm));
    }

    #[test]
    fn test_detect_unknown_project() {
        let tmp = tempfile::tempdir().unwrap();
        let runner = TestRunner::detect(tmp.path());
        assert!(matches!(runner, TestRunner::Unknown));
    }

    #[test]
    fn test_parse_cargo_test_output_all_pass() {
        let output = "test result: ok. 5 passed; 0 failed; 0 ignored";
        let result = TestRunResult::new(output.to_string());
        assert!(result.success);
        assert_eq!(result.passed, 5);
        assert_eq!(result.failed, 0);
    }

    #[test]
    fn test_parse_cargo_test_output_with_failures() {
        let output = "test result: FAILED. 3 passed; 2 failed; 0 ignored";
        let result = TestRunResult::new(output.to_string());
        assert!(!result.success);
        assert_eq!(result.passed, 3);
        assert_eq!(result.failed, 2);
    }
}
