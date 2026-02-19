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
