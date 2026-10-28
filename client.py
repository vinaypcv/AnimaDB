import socket
import struct
import time
import random

class AnimaDBStressClient:
    def __init__(self, host="127.0.0.1", port=8080):
        self.target_address = (host, port)
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        print(f"[*] AnimaDB Driver bound. Target cluster: {host}:{port}")

    def send_agent_payload(self, agent_id: int, vector_manifold: list, op_code: str):
        if len(vector_manifold) != 128:
            raise ValueError("Vector manifold array must contain exactly 128 float entities.")
        
        op_bytes = op_code.encode('utf-8')[:256]
        payload_len = len(op_bytes)
        op_bytes = op_bytes.ljust(256, b'\0')

        struct_format = f"<Q I 128f 256s"
        try:
            packed_data = struct.pack(
                struct_format, 
                agent_id, 
                payload_len, 
                *vector_manifold, 
                op_bytes
            )
            self.sock.sendto(packed_data, self.target_address)
        except Exception as e:
            print(f"[-] Frame dropped during transport translation: {e}")

    def run_stress_test(self, duration_secs=3, target_fps=50):
        print(f"[*] Launching E2E validation matrix stream ({target_fps} frames/sec)...")
        start_time = time.time()
        frames_sent = 0
        interval = 1.0 / target_fps

        while time.time() - start_time < duration_secs:
            loop_start = time.time()
            frames_sent += 1
            
            if frames_sent % 5 != 0:
                agent_id = 0xAAFF_0000 + random.randint(1, 9999)
                clean_vector = [random.uniform(0.01, 0.2) for _ in range(128)]
                op_code = f"TX_STREAM_OP_VALIDATION_ID_{frames_sent}\0"
                self.send_agent_payload(agent_id, clean_vector, op_code)
            else:
                agent_id = 0xBAD_DA7A
                hostile_vector = [5.5 for _ in range(128)] 
                op_code = "ATTEMPT_INJECT_SYSTEM_OVERRIDE\0"
                self.send_agent_payload(agent_id, hostile_vector, op_code)

            elapsed = time.time() - loop_start
            if elapsed < interval:
                time.sleep(interval - elapsed)

        print(f"[+] Simulation Complete. Total structured wire transactions sent: {frames_sent}")

if __name__ == "__main__":
    client = AnimaDBStressClient()
    client.run_stress_test(duration_secs=3, target_fps=50)
