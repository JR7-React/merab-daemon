use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use merab_core::MerabError;
use merab_store::jobs::Job;
use merab_store::Database;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSummary {
    pub id: String,
    pub task: String,
    pub status: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub duration_secs: Option<u64>,
}

pub struct JobManager {
    db: Arc<Mutex<Database>>,
    logs_dir: PathBuf,
}

impl JobManager {
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        let logs_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("merab")
            .join("logs");
        
        if let Err(e) = fs::create_dir_all(&logs_dir) {
            tracing::warn!(error = %e, "Failed to create logs directory");
        }

        Self { db, logs_dir }
    }

    pub async fn submit(&self, task: String) -> Result<String, MerabError> {
        let job_id = format!("job-{}", &Uuid::new_v4().to_string()[..8]);
        let log_file = self.logs_dir.join(format!("{}.log", job_id));
        let log_file_str = log_file.to_string_lossy().to_string();
        
        // Create log file
        if let Err(e) = File::create(&log_file) {
            tracing::warn!(error = %e, "Failed to create log file");
        }

        // Create job in database
        {
            let db = self.db.lock().await;
            db.create_job(&job_id, &task, &log_file_str)
                .map_err(|e| MerabError::Store(e.to_string()))?;
        }

        Ok(job_id)
    }

    pub async fn start(&self, job_id: &str) -> Result<(), MerabError> {
        let db = self.db.lock().await;
        db.start_job(job_id)
            .map_err(|e| MerabError::Store(e.to_string()))
    }

    pub async fn complete(&self, job_id: &str, result: &str) -> Result<(), MerabError> {
        let db = self.db.lock().await;
        db.complete_job(job_id, result)
            .map_err(|e| MerabError::Store(e.to_string()))
    }

    pub async fn fail(&self, job_id: &str, error: &str) -> Result<(), MerabError> {
        let db = self.db.lock().await;
        db.fail_job(job_id, error)
            .map_err(|e| MerabError::Store(e.to_string()))
    }

    pub async fn get_status(&self, job_id: &str) -> Result<Option<JobSummary>, MerabError> {
        let db = self.db.lock().await;
        let job = db.get_job(job_id)
            .map_err(|e| MerabError::Store(e.to_string()))?;
        
        Ok(job.map(|j| self.job_to_summary(j)))
    }

    pub async fn get_log(&self, job_id: &str) -> Result<String, MerabError> {
        let db = self.db.lock().await;
        let job = db.get_job(job_id)
            .map_err(|e| MerabError::Store(e.to_string()))?;
        
        match job {
            Some(j) => {
                match fs::read_to_string(&j.log_file) {
                    Ok(content) => Ok(content),
                    Err(e) => Ok(format!("(log file not found: {})", e)),
                }
            }
            None => Err(MerabError::NotFound(format!("Job {} not found", job_id))),
        }
    }

    pub async fn list(&self, limit: u32) -> Result<Vec<JobSummary>, MerabError> {
        let db = self.db.lock().await;
        let jobs = db.list_jobs(limit, false)
            .map_err(|e| MerabError::Store(e.to_string()))?;
        
        Ok(jobs.into_iter().map(|j| self.job_to_summary(j)).collect())
    }

    pub async fn cancel(&self, job_id: &str) -> Result<bool, MerabError> {
        let db = self.db.lock().await;
        db.cancel_job(job_id)
            .map_err(|e| MerabError::Store(e.to_string()))
    }

    pub async fn cleanup(&self) -> Result<u64, MerabError> {
        let db = self.db.lock().await;
        db.cleanup_old_jobs(7)
            .map_err(|e| MerabError::Store(e.to_string()))
    }

    fn job_to_summary(&self, job: Job) -> JobSummary {
        let duration_secs = match (job.started_at, job.finished_at) {
            (Some(start), Some(end)) => {
                Some((end - start).num_seconds() as u64)
            }
            (Some(start), None) => {
                Some((Utc::now() - start).num_seconds() as u64)
            }
            _ => None,
        };

        JobSummary {
            id: job.id,
            task: job.task,
            status: job.status.to_string(),
            created_at: job.created_at.format("%Y-%m-%d %H:%M").to_string(),
            started_at: job.started_at.map(|d| d.format("%H:%M").to_string()),
            duration_secs,
        }
    }

    pub fn append_log(&self, job_id: &str, line: &str) {
        let log_path = self.logs_dir.join(format!("{}.log", job_id));
        if let Ok(mut file) = OpenOptions::new().append(true).open(&log_path) {
            let _ = writeln!(file, "{}", line);
        }
    }
}
