#include "engine.hpp"
#include <sys/socket.h>
#include <netinet/in.h>
#include <unistd.h>
#include <fcntl.h>
#include <iostream>
#include <vector>
#include <cstring>

class KernelBypassIngress {
private:
    int server_fd;
    void* shared_fabric;

public:
    KernelBypassIngress(void* fabric, int port) : server_fd(-1), shared_fabric(fabric) {
        server_fd = socket(AF_INET, SOCK_DGRAM | SOCK_NONBLOCK, 0);
        if (server_fd < 0) {
            std::cerr << "[FATAL] Failed to allocate high-speed socket wire.\n";
            return;
        }

        int opt = 1;
        setsockopt(server_fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));

        int rcvbuf = 16 * 1024 * 1024;
        setsockopt(server_fd, SOL_SOCKET, SO_RCVBUF, &rcvbuf, sizeof(rcvbuf));

        sockaddr_in address{};
        address.sin_family = AF_INET;
        address.sin_addr.s_addr = INADDR_ANY;
        address.sin_port = htons(port);

        if (bind(server_fd, (struct sockaddr*)&address, sizeof(address)) < 0) {
            std::cerr << "[FATAL] Ingress wire binding failure on port " << port << "\n";
        }
    }

    uint32_t poll_batch(uint32_t max_batch) {
        if (server_fd < 0) return 0;

        uint32_t ingested = 0;
#if defined(__linux__)
        std::vector<mmsghdr> msgvec(max_batch);
        std::vector<iovec> iovecs(max_batch);
        std::vector<NetworkFrame> buffers(max_batch);
        std::vector<sockaddr_in> addrs(max_batch);

        std::memset(msgvec.data(), 0, sizeof(mmsghdr) * max_batch);
        for (size_t i = 0; i < max_batch; ++i) {
            iovecs[i].iov_base = &buffers[i];
            iovecs[i].iov_len = sizeof(NetworkFrame);
            msgvec[i].msg_hdr.msg_iov = &iovecs[i];
            msgvec[i].msg_hdr.msg_iovlen = 1;
            msgvec[i].msg_hdr.msg_name = &addrs[i];
            msgvec[i].msg_hdr.msg_namelen = sizeof(sockaddr_in);
        }

        timespec timeout{0, 0};
        int pkts = recvmmsg(server_fd, msgvec.data(), max_batch, MSG_DONTWAIT, &timeout);
        if (pkts > 0) {
            for (int i = 0; i < pkts; ++i) {
                if (msgvec[i].msg_len == sizeof(NetworkFrame)) {
                    const NetworkFrame& frame = buffers[i];
                    if (fabric_push(shared_fabric, frame.agent_id, frame.vector_manifold, frame.raw_op_code, frame.payload_len)) {
                        ingested++;
                    }
                }
            }
        }
#else
        for (uint32_t i = 0; i < max_batch; ++i) {
            NetworkFrame buffer;
            sockaddr_in client_addr{};
            socklen_t addr_len = sizeof(client_addr);
            ssize_t bytes = recvfrom(server_fd, &buffer, sizeof(buffer), MSG_DONTWAIT, (struct sockaddr*)&client_addr, &addr_len);
            if (bytes == sizeof(buffer)) {
                if (fabric_push(shared_fabric, buffer.agent_id, buffer.vector_manifold, buffer.raw_op_code, buffer.payload_len)) {
                    ingested++;
                }
            } else {
                break;
            }
        }
#endif
        return ingested;
    }

    ~KernelBypassIngress() {
        if (server_fd >= 0) close(server_fd);
    }
};

extern "C" {
    void* init_network_ingress(void* fabric, int port) {
        return static_cast<void*>(new KernelBypassIngress(fabric, port));
    }

    uint32_t poll_network_ingress(void* ingress, uint32_t max_batch) {
        return static_cast<KernelBypassIngress*>(ingress)->poll_batch(max_batch);
    }

    void free_network_ingress(void* ingress) {
        delete static_cast<KernelBypassIngress*>(ingress);
    }
}
