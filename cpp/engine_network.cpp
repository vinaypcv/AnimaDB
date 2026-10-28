#include "engine.hpp"
#include <sys/socket.h>
#include <netinet/in.h>
#include <unistd.h>
#include <fcntl.h>
#include <iostream>

class KernelBypassIngress {
private:
    int server_fd;
    void* shared_fabric;

public:
    KernelBypassIngress(void* fabric, int port) : shared_fabric(fabric) {
        server_fd = socket(AF_INET, SOCK_DGRAM | SOCK_NONBLOCK, 0);
        if (server_fd < 0) {
            std::cerr << "[FATAL] Failed to allocate socket.\n";
            return;
        }

        int opt = 1;
        setsockopt(server_fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));

        sockaddr_in address{};
        address.sin_family = AF_INET;
        address.sin_addr.s_addr = INADDR_ANY;
        address.sin_port = htons(port);

        if (bind(server_fd, (struct sockaddr*)&address, sizeof(address)) < 0) {
            std::cerr << "[FATAL] Ingress wire binding failure on port " << port << "\n";
        }
    }

    void poll_network_hardware() {
        sockaddr_in client_addr{};
        socklen_t addr_len = sizeof(client_addr);

        struct PacketPayload {
            uint64_t agent_id;
            uint32_t payload_len;
            float vector[128];
            char op[256];
        } __attribute__((packed)) buffer;

        ssize_t bytes_received = recvfrom(server_fd, &buffer, sizeof(buffer), 0,
                                          (struct sockaddr*)&client_addr, &addr_len);

        if (bytes_received > 0 && bytes_received == sizeof(buffer)) {
            fabric_push(shared_fabric, buffer.agent_id, buffer.vector, buffer.op, buffer.payload_len);
        }
    }

    ~KernelBypassIngress() {
        if (server_fd >= 0) close(server_fd);
    }
};

extern "C" {
    void* init_network_ingress(void* fabric, int port) {
        return static_cast<void*>(new KernelBypassIngress(fabric, port));
    }

    void poll_network_ingress(void* ingress) {
        static_cast<KernelBypassIngress*>(ingress)->poll_network_hardware();
    }

    void free_network_ingress(void* ingress) {
        delete static_cast<KernelBypassIngress*>(ingress);
    }
}
