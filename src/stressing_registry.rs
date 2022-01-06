use atomic_counter::{AtomicCounter, RelaxedCounter};
use metrics::gauge;

#[derive(Debug)]
pub struct MetricRegistry {
    running_tasks: RelaxedCounter,
    exited_tasks: RelaxedCounter,
    invalid_pubacks: RelaxedCounter,
    timeout_pubacks: RelaxedCounter,
    publish_packets: RelaxedCounter,
}

impl MetricRegistry {
    pub fn new() -> MetricRegistry {
        return MetricRegistry {
            running_tasks: RelaxedCounter::new(0),
            exited_tasks: RelaxedCounter::new(0),
            invalid_pubacks: RelaxedCounter::new(0),
            timeout_pubacks: RelaxedCounter::new(0),
            publish_packets: RelaxedCounter::new(0),
        };
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

    pub fn publish_packets_inc(self: &MetricRegistry) {
        self.publish_packets.inc();
    }

    pub fn update(self: &MetricRegistry, labels: &[(String, String); 1]) {
        gauge!("running_tasks", self.running_tasks.get() as f64, labels);
        gauge!("exited_tasks", self.exited_tasks.get() as f64, labels);
        gauge!("invalid_pubacks", self.invalid_pubacks.get() as f64, labels);
        gauge!("timeout_pubacks", self.timeout_pubacks.get() as f64, labels);
        gauge!("publish_packets", self.publish_packets.get() as f64, labels);
    }
}
