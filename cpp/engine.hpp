#pragma once
#include <cstdint>
#include <array>

extern "C" {
    struct alignas(64) NetworkFrame {
        uint64_t agent_id;
        uint32_t payload_len;
        std::array<float, 128> vector_manifold;
        std::array<char, 256> raw_op_code;
    };

    void* init_memory_fabric();
    bool fabric_push(void* fabric, uint64_t agent, const float* vec_ptr, const char* op, uint32_t len);
    NetworkFrame* fabric_pop(void* fabric);
    void free_memory_fabric(void* fabric);

    void* init_network_ingress(void* fabric, int port);
    void poll_network_ingress(void* ingress);
    void free_network_ingress(void* ingress);
}
