use std::collections::HashMap;
use std::ffi::c_char;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[repr(C, packed(1))]
pub struct NetworkFrame {
    pub agent_id: u64,
    pub payload_len: u32,
    pub vector_manifold: [f32; 128],
    pub raw_op_code: [c_char; 256],
}

extern "C" {
    fn init_memory_fabric() -> *mut std::ffi::c_void;
    fn fabric_pop(fabric: *mut std::ffi::c_void) -> *mut NetworkFrame;
    fn free_memory_fabric(fabric: *mut std::ffi::c_void);

    fn init_network_ingress(fabric: *mut std::ffi::c_void, port: i32) -> *mut std::ffi::c_void;
    fn poll_network_ingress(ingress: *mut std::ffi::c_void, max_batch: u32) -> u32;
    fn free_network_ingress(ingress: *mut std::ffi::c_void);
}

// Server-Side Telemetry Counters
pub static STAT_INGESTED: AtomicU64 = AtomicU64::new(0);
pub static STAT_ACCEPTED: AtomicU64 = AtomicU64::new(0);
pub static STAT_DROPPED: AtomicU64 = AtomicU64::new(0);
pub static STAT_BYTES: AtomicU64 = AtomicU64::new(0);

pub struct SemanticTopologyAuditor {
    pub max_l2_norm: f32,
    pub max_infinity_norm: f32,
}

impl SemanticTopologyAuditor {
    pub fn new(max_l2_norm: f32, max_infinity_norm: f32) -> Self {
        Self { max_l2_norm, max_infinity_norm }
    }

    pub fn audit_frame(&self, manifold: &[f32; 128], payload: &[u8]) -> bool {
        // 1. Math Check: L2 Norm Euclidean Distance
        let l2_norm_sq: f32 = manifold.iter().map(|x| x * x).sum();
        let l2_norm = l2_norm_sq.sqrt();
        if l2_norm > self.max_l2_norm || l2_norm.is_nan() {
            return false;
        }

        // 2. Math Check: Infinity Norm Boundary
        let inf_norm = manifold.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
        if inf_norm > self.max_infinity_norm {
            return false;
        }

        // 3. Payload Sanity: Check for illegal binary escape characters
        if payload.iter().any(|&b| b == 0x7F || (b < 0x20 && b != 0x00 && b != 0x0A && b != 0x0D)) {
            return false;
        }

        true
    }
}

pub struct WriteAheadLog {
    file: Arc<Mutex<File>>,
}

impl WriteAheadLog {
    pub fn open(path: &str) -> std::io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            file: Arc::new(Mutex::new(file)),
        })
    }

    pub fn append(&self, agent_id: u64, payload: &[u8]) -> std::io::Result<()> {
        let mut guard = self.file.lock().unwrap();
        guard.write_all(&agent_id.to_le_bytes())?;
        let len = payload.len() as u32;
        guard.write_all(&len.to_le_bytes())?;
        guard.write_all(payload)?;
        guard.flush()
    }
}

pub struct SandboxedExecutionEngine {
    registers: Arc<Mutex<HashMap<u64, f64>>>,
}

impl SandboxedExecutionEngine {
    pub fn new() -> Self {
        Self {
            registers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn execute(&self, agent_id: u64, op_bytes: &[u8]) -> bool {
        let op_str = match std::str::from_utf8(op_bytes) {
            Ok(s) => s.trim_matches('\0'),
            Err(_) => return false,
        };

        let mut regs = self.registers.lock().unwrap();
        let entry = regs.entry(agent_id).or_insert(0.0);

        if op_str.starts_with("SET_STATE") {
            *entry = 1.0;
        } else if op_str.starts_with("ACCUM_VAL") {
            *entry += 0.5;
        } else if op_str.starts_with("SYNC_HASH") {
            *entry *= 1.001;
        } else {
            *entry += 0.01;
        }
        true
    }

    pub fn get_register(&self, agent_id: u64) -> f64 {
        *self.registers.lock().unwrap().get(&agent_id).unwrap_or(&0.0)
    }
}

fn get_rss_memory_mb() -> f64 {
    if let Ok(statm) = std::fs::read_to_string("/proc/self/statm") {
        let parts: Vec<&str> = statm.split_whitespace().collect();
        if parts.len() >= 2 {
            if let Ok(pages) = parts[1].parse::<u64>() {
                let page_size = 4096;
                return (pages * page_size) as f64 / (1024.0 * 1024.0);
            }
        }
    }
    0.0
}

fn main() {
    println!("===========================================================");
    println!("    ANIMADB: HIGH-THROUGHPUT ENGINE ENCLAVE OPERATIONAL    ");
    println!("===========================================================");

    let fabric_raw = unsafe { init_memory_fabric() };
    if fabric_raw.is_null() {
        panic!("Initialization Failure: Lock-free storage allocation failed.");
    }

    let ingress_raw = unsafe { init_network_ingress(fabric_raw, 8080) };
    if ingress_raw.is_null() {
        panic!("Initialization Failure: Network socket binding failed.");
    }

    struct SharedPointers {
        fabric: *mut std::ffi::c_void,
        ingress: *mut std::ffi::c_void,
    }
    unsafe impl Send for SharedPointers {}
    unsafe impl Sync for SharedPointers {}

    let system = Arc::new(SharedPointers { fabric: fabric_raw, ingress: ingress_raw });
    let shutdown = Arc::new(AtomicBool::new(false));

    let wal = Arc::new(WriteAheadLog::open("animadb_store.wal").expect("WAL initialization failed"));

    // Thread 1: Ingress Receiver
    let net_system = Arc::clone(&system);
    let net_shutdown = Arc::clone(&shutdown);
    let net_thread = thread::spawn(move || {
        while !net_shutdown.load(Ordering::Relaxed) {
            let count = unsafe { poll_network_ingress(net_system.ingress, 64) };
            if count == 0 {
                thread::yield_now();
            } else {
                STAT_INGESTED.fetch_add(count as u64, Ordering::Relaxed);
            }
        }
    });

    // Thread 2: Security & Compute Processor
    let proc_system = Arc::clone(&system);
    let proc_shutdown = Arc::clone(&shutdown);
    let proc_wal = Arc::clone(&wal);
    let proc_thread = thread::spawn(move || {
        let auditor = SemanticTopologyAuditor::new(10.0, 3.0);
        let engine = SandboxedExecutionEngine::new();

        while !proc_shutdown.load(Ordering::Relaxed) {
            unsafe {
                let frame_ptr = fabric_pop(proc_system.fabric);
                if !frame_ptr.is_null() {
                    let frame = &*frame_ptr;
                    let payload_len = (frame.payload_len as usize).min(256);
                    let raw_op_code = frame.raw_op_code;
                    let raw_bytes = &raw_op_code[..payload_len];
                    let raw_u8_bytes = &*(raw_bytes as *const [c_char] as *const [u8]);

                    STAT_BYTES.fetch_add(std::mem::size_of::<NetworkFrame>() as u64, Ordering::Relaxed);

                    // Copy manifold array to stack to prevent misaligned references
                    let vector_manifold = frame.vector_manifold;

                    if !auditor.audit_frame(&vector_manifold, raw_u8_bytes) {
                        STAT_DROPPED.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }

                    if engine.execute(frame.agent_id, raw_u8_bytes) {
                        let _ = proc_wal.append(frame.agent_id, raw_u8_bytes);
                        STAT_ACCEPTED.fetch_add(1, Ordering::Relaxed);
                    } else {
                        STAT_DROPPED.fetch_add(1, Ordering::Relaxed);
                    }
                } else {
                    thread::sleep(Duration::from_micros(100));
                }
            }
        }
    });

    let start_time = Instant::now();
    let runtime_limit = Duration::from_secs(10);

    while start_time.elapsed() < runtime_limit {
        thread::sleep(Duration::from_secs(1));
        let ingested = STAT_INGESTED.load(Ordering::Relaxed);
        let accepted = STAT_ACCEPTED.load(Ordering::Relaxed);
        let dropped = STAT_DROPPED.load(Ordering::Relaxed);
        let rss_mb = get_rss_memory_mb();

        println!(
            "[TELEMETRY] Ingested: {} | Accepted: {} | Dropped: {} | RSS Memory: {:.2} MB",
            ingested, accepted, dropped, rss_mb
        );
    }

    println!("\n[SHUTDOWN] Terminating engine threads...");
    shutdown.store(true, Ordering::Relaxed);

    net_thread.join().unwrap();
    proc_thread.join().unwrap();

    unsafe {
        free_network_ingress(system.ingress);
        free_memory_fabric(system.fabric);
    }

    println!("===========================================================");
    println!("               ANIMADB SHUTDOWN COMPLETE                   ");
    println!("===========================================================");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_topology_auditor() {
        let auditor = SemanticTopologyAuditor::new(10.0, 3.0);
        let clean_vector = [0.1f32; 128];
        let clean_payload = b"AGENT_SWARM_STATE_UPDATE\0";

        assert!(auditor.audit_frame(&clean_vector, clean_payload));

        let hostile_vector = [5.0f32; 128];
        assert!(!auditor.audit_frame(&hostile_vector, clean_payload));

        let malicious_payload = &[0x7F, 0x01, 0x02];
        assert!(!auditor.audit_frame(&clean_vector, malicious_payload));
    }

    #[test]
    fn test_sandboxed_execution_engine() {
        let engine = SandboxedExecutionEngine::new();
        let agent_id = 0xAAFF1234;

        assert!(engine.execute(agent_id, b"SET_STATE\0"));
        assert_eq!(engine.get_register(agent_id), 1.0);

        assert!(engine.execute(agent_id, b"ACCUM_VAL\0"));
        assert_eq!(engine.get_register(agent_id), 1.5);
    }

    #[test]
    fn test_write_ahead_log() {
        let wal_path = "animadb_test.wal";
        let wal = WriteAheadLog::open(wal_path).unwrap();
        assert!(wal.append(0x1234, b"TEST_PAYLOAD").is_ok());
        let _ = std::fs::remove_file(wal_path);
    }
}
