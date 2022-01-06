use atomic_counter::{AtomicCounter, RelaxedCounter};
use metrics::gauge;

#[derive(Debug)]
pub struct MetricRegistry {
    pub running_tasks: RelaxedCounter,
    pub exited_tasks: RelaxedCounter,
    pub invalid_pubacks: RelaxedCounter,
    pub timeout_pubacks: RelaxedCounter,
    pub publish_packets: RelaxedCounter,
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

    pub fn update(self: &MetricRegistry, labels: &[(String, String); 1]) {
        gauge!("running_tasks", self.running_tasks.get() as f64, labels);
        gauge!("exited_tasks", self.exited_tasks.get() as f64, labels);
        gauge!("invalid_pubacks", self.invalid_pubacks.get() as f64, labels);
        gauge!("timeout_pubacks", self.timeout_pubacks.get() as f64, labels);
        gauge!("publish_packets", self.publish_packets.get() as f64, labels);
    }
}
