#pragma once
#include <cstdint>
#include <array>
#include <cstddef>

#pragma pack(push, 1)
struct NetworkFrame {
    uint64_t agent_id;
    uint32_t payload_len;
    float vector_manifold[128];
    char raw_op_code[256];
};
#pragma pack(pop)

extern "C" {
    void* init_memory_fabric();
    bool fabric_push(void* fabric, uint64_t agent, const float* vec_ptr, const char* op, uint32_t len);
    NetworkFrame* fabric_pop(void* fabric);
    void free_memory_fabric(void* fabric);

    void* init_network_ingress(void* fabric, int port);
    uint32_t poll_network_ingress(void* ingress, uint32_t max_batch);
    void free_network_ingress(void* ingress);
}