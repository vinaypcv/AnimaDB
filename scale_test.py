import socket
import struct
import multiprocessing
import time
import random

STRUCT_FORMAT = "<Q I 128f 256s"

def execute_traffic_flood(worker_id, host, port, duration, target_fps):
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    target_address = (host, port)
    
    start_time = time.time()
    frames_sent = 0
    interval = 1.0 / target_fps
    
    clean_op = b"SET_STATE\0".ljust(256, b'\0')
    malicious_op = b"EXPLOIT_OVERRIDE\0".ljust(256, b'\0')

    while time.time() - start_time < duration:
        loop_start = time.time()
        frames_sent += 1
        
        if frames_sent % 10 != 0:
            agent_id = 0xAAFF0000 + (worker_id * 10000) + (frames_sent % 9999)
            vector = [random.uniform(0.01, 0.05) for _ in range(128)]
            packed_data = struct.pack(STRUCT_FORMAT, agent_id, 10, *vector, clean_op)
        else:
            agent_id = 0xBAD_DA7A
            vector = [8.5 for _ in range(128)]
            packed_data = struct.pack(STRUCT_FORMAT, agent_id, 17, *vector, malicious_op)
        
        try:
            sock.sendto(packed_data, target_address)
        except Exception:
            pass
            
        elapsed = time.time() - loop_start
        if elapsed < interval:
            time.sleep(interval - elapsed)

    return frames_sent

def run_scale_test(host="127.0.0.1", port=8080, duration=6):
    num_workers = max(1, multiprocessing.cpu_count() - 1)
    target_fps = 5000
    
    print(f"===========================================================")
    print(f"Target: {host}:{port} | Active CPU Workers: {num_workers}")
    print(f"===========================================================")
    
    pool = multiprocessing.Pool(processes=num_workers)
    results = []
    
    start_wall_time = time.time()
    for i in range(num_workers):
        res = pool.apply_async(execute_traffic_flood, (i, host, port, duration, target_fps))
        results.append(res)
        
    pool.close()
    pool.join()
    
    total_frames = sum([r.get() for r in results])
    elapsed = time.time() - start_wall_time
    
    print(f"\n[+] Total Dispatched: {total_frames:,} Packets")
    print(f"[+] Client Wire Throughput: {int(total_frames / elapsed):,} Packets / Sec")
    print(f"===========================================================")

if __name__ == "__main__":
    run_scale_test(duration=6)