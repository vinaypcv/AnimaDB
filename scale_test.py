import socket
import struct
import multiprocessing
import time
import random

STRUCT_FORMAT = "<Q I 128f 256s"

def execute_traffic_flood(worker_id, host, port, duration, target_fps_per_worker):
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    target_address = (host, port)
    
    start_time = time.time()
    frames_sent = 0
    interval = 1.0 / target_fps_per_worker
    
    clean_op = b"AGENT_SWARM_STATE_UPDATE\0".ljust(256, b'\0')
    malicious_op = b"EXPLOIT_PAYLOAD_OVERRIDE\0".ljust(256, b'\0')

    while time.time() - start_time < duration:
        loop_start = time.time()
        frames_sent += 1
        
        if frames_sent % 10 != 0:
            agent_id = 0xAAFF0000 + (worker_id * 10000) + (frames_sent % 9999)
            vector = [random.uniform(0.01, 0.1) for _ in range(128)]
            packed_data = struct.pack(STRUCT_FORMAT, agent_id, 24, *vector, clean_op)
        else:
            agent_id = 0xBAD_DA7A
            vector = [8.5 for _ in range(128)]
            packed_data = struct.pack(STRUCT_FORMAT, agent_id, 24, *vector, malicious_op)
        
        try:
            sock.sendto(packed_data, target_address)
        except Exception:
            pass
            
        elapsed = time.time() - loop_start
        if elapsed < interval:
            time.sleep(interval - elapsed)

    return frames_sent

def run_production_scale_test(host="127.0.0.1", port=8080, duration=5):
    num_workers = max(1, multiprocessing.cpu_count() - 1)
    target_fps_per_worker = 1000
    
    print(f"===========================================================")
    print(f"Target: {host}:{port} | Workers: {num_workers} | Duration: {duration}s")
    print(f"===========================================================")
    
    pool = multiprocessing.Pool(processes=num_workers)
    results = []
    
    start_wall_time = time.time()
    for i in range(num_workers):
        res = pool.apply_async(execute_traffic_flood, (i, host, port, duration, target_fps_per_worker))
        results.append(res)
        
    pool.close()
    pool.join()
    
    total_frames = sum([r.get() for r in results])
    elapsed_wall_time = time.time() - start_wall_time
    
    print(f"\n[+] Production Scale Test Completed.")
    print(f"Total Dispatched: {total_frames:,} Packets")
    print(f"Sustained Throughput: {int(total_frames / elapsed_wall_time):,} Packets / Sec")
    print(f"===========================================================")

if __name__ == "__main__":
    run_production_scale_test(duration=5)
