use atomic_counter::{AtomicCounter, RelaxedCounter};
use metrics::gauge;
use std::fmt;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub enum TaskStatus {
    Run,
    Stop,
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug)]
pub struct MetricRegistry {
    running_tasks: RelaxedCounter,
    exited_tasks: RelaxedCounter,
    invalid_pubacks: RelaxedCounter,
    timeout_pubacks: RelaxedCounter,
    publish_packets: RelaxedCounter,
    established_connection: AtomicU32,
    ongoing_connection: AtomicU32,
    task_name: String,
    task_status: Mutex<TaskStatus>,
}

impl MetricRegistry {
    pub fn new(task_name: String) -> MetricRegistry {
        return MetricRegistry {
            running_tasks: RelaxedCounter::new(0),
            exited_tasks: RelaxedCounter::new(0),
            invalid_pubacks: RelaxedCounter::new(0),
            timeout_pubacks: RelaxedCounter::new(0),
            publish_packets: RelaxedCounter::new(0),
            established_connection: AtomicU32::new(0),
            ongoing_connection: AtomicU32::new(0),
            task_name: task_name,
            task_status: Mutex::new(TaskStatus::Stop),
        };
    }
    pub fn start_task(self: &MetricRegistry) {
        self.task_status
            .lock()
            .unwrap()
            .clone_from(&TaskStatus::Run);
    }

    pub fn task_stopped(self: &MetricRegistry) {
        self.task_status
            .lock()
            .unwrap()
            .clone_from(&TaskStatus::Stop);
        self.established_connection_reset();
        self.ongoing_connection_reset();
    }

    pub fn running_tasks_inc(self: &MetricRegistry) {
        self.running_tasks.inc();
    }

    pub fn exited_tasks_inc(self: &MetricRegistry) {
        self.exited_tasks.inc();
    }

    pub fn invalid_pubacks_inc(self: &MetricRegistry) {
        self.invalid_pubacks.inc();
    }

    pub fn timeout_pubacks_inc(self: &MetricRegistry) {
        self.timeout_pubacks.inc();
    }

    pub fn ongoing_connection_inc(self: &MetricRegistry) {
        self.ongoing_connection.fetch_add(1, Ordering::Relaxed);
    }

    pub fn ongoing_connection_decr(self: &MetricRegistry) {
        self.ongoing_connection.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn ongoing_connection_reset(self: &MetricRegistry) {
        self.ongoing_connection.fetch_min(0, Ordering::Relaxed);
    }

    pub fn established_connection_inc(self: &MetricRegistry) {
        self.established_connection.fetch_add(1, Ordering::Relaxed);
    }

    pub fn established_connection_decr(self: &MetricRegistry) {
        self.established_connection.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn established_connection_reset(self: &MetricRegistry) {
        self.established_connection.fetch_min(0, Ordering::Relaxed);
    }

    pub fn publish_packets_inc(self: &MetricRegistry) {
        self.publish_packets.inc();
    }

    pub fn update(self: &MetricRegistry, labels: &[(String, String); 1]) {
        let mut new_labels = vec![];
        for label in labels.iter() {
            new_labels.push((label.0.clone(), label.1.clone()));
        }

        new_labels.push(("task_name".to_string(), self.task_name.clone()));
        new_labels.push((
            "task_status".to_string(),
            self.task_status.lock().unwrap().to_string(),
        ));

        gauge!(
            "running_tasks",
            self.running_tasks.get() as f64,
            &new_labels
        );
        gauge!("exited_tasks", self.exited_tasks.get() as f64, &new_labels);
        gauge!(
            "invalid_pubacks",
            self.invalid_pubacks.get() as f64,
            &new_labels
        );
        gauge!(
            "timeout_pubacks",
            self.timeout_pubacks.get() as f64,
            &new_labels
        );
        gauge!(
            "publish_packets",
            self.publish_packets.get() as f64,
            &new_labels
        );
        gauge!(
            "established_connection",
            self.established_connection.load(Ordering::Relaxed) as f64,
            &new_labels
        );
        gauge!(
            "ongoing_connection",
            self.ongoing_connection.load(Ordering::Relaxed) as f64,
            &new_labels
        )
    }
}
