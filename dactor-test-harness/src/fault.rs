use dashmap::DashMap;
use std::sync::atomic::AtomicU32;

pub struct FaultEntry {
    pub fault_type: String,
    pub target: String,
    pub duration_ms: u64,
    pub count: AtomicU32,
}

pub struct FaultInjector {
    faults: DashMap<String, FaultEntry>,
}

impl FaultInjector {
    pub fn new() -> Self {
        Self {
            faults: DashMap::new(),
        }
    }

    pub fn add_fault(&self, fault_type: &str, target: &str, duration_ms: u64, count: u32) {
        let key = format!("{}:{}", fault_type, target);
        self.faults.insert(
            key,
            FaultEntry {
                fault_type: fault_type.to_string(),
                target: target.to_string(),
                duration_ms,
                count: AtomicU32::new(count),
            },
        );
    }

    pub fn clear_all(&self) {
        self.faults.clear();
    }

    pub fn has_fault(&self, fault_type: &str, target: &str) -> bool {
        let key = format!("{}:{}", fault_type, target);
        self.faults.contains_key(&key)
    }

    pub fn active_faults(&self) -> Vec<String> {
        self.faults.iter().map(|e| e.key().clone()).collect()
    }
}

impl Default for FaultInjector {
    fn default() -> Self {
        Self::new()
    }
}
