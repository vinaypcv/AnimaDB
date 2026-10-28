#include "engine.hpp"
#include <atomic>
#include <vector>
#include <algorithm>
#include <cstring>

class AtomicMemoryFabric {
private:
    static constexpr size_t BUFFER_SIZE = 8192;
    alignas(64) std::vector<NetworkFrame> ring_buffer;
    alignas(64) std::atomic<size_t> write_index{0};
    alignas(64) std::atomic<size_t> read_index{0};

public:
    AtomicMemoryFabric() {
        ring_buffer.resize(BUFFER_SIZE);
    }

    bool push_frame(uint64_t agent, const float* vec_ptr, const char* op, uint32_t len) {
        size_t current_write = write_index.load(std::memory_order_relaxed);
        size_t next_write = (current_write + 1) % BUFFER_SIZE;

        if (next_write == read_index.load(std::memory_order_acquire)) {
            return false; // Backpressure: Ring buffer saturated
        }

        NetworkFrame& frame = ring_buffer[current_write];
        frame.agent_id = agent;
        frame.payload_len = std::min(len, 256U);
        
        std::memcpy(frame.vector_manifold, vec_ptr, 128 * sizeof(float));
        std::memcpy(frame.raw_op_code, op, frame.payload_len);
        if (frame.payload_len < 256) {
            frame.raw_op_code[frame.payload_len] = '\0';
        }

        write_index.store(next_write, std::memory_order_release);
        return true;
    }

    NetworkFrame* pop_frame() {
        size_t current_read = read_index.load(std::memory_order_relaxed);
        if (current_read == write_index.load(std::memory_order_acquire)) {
            return nullptr;
        }

        NetworkFrame* frame = &ring_buffer[current_read];
        read_index.store((current_read + 1) % BUFFER_SIZE, std::memory_order_release);
        return frame;
    }
};

extern "C" {
    void* init_memory_fabric() {
        return static_cast<void*>(new AtomicMemoryFabric());
    }

    bool fabric_push(void* fabric, uint64_t agent, const float* vec_ptr, const char* op, uint32_t len) {
        return static_cast<AtomicMemoryFabric*>(fabric)->push_frame(agent, vec_ptr, op, len);
    }

    NetworkFrame* fabric_pop(void* fabric) {
        return static_cast<AtomicMemoryFabric*>(fabric)->pop_frame();
    }

    void free_memory_fabric(void* fabric) {
        delete static_cast<AtomicMemoryFabric*>(fabric);
    }
}