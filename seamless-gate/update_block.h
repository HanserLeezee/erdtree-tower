#pragma once
#include <cctype>
#include <cstdint>
#include <cstring>

namespace tower_update_block {

inline bool EqualsIgnoreCaseAscii(const char* a, const char* b) noexcept {
    if (!a || !b) return false;
    while (*a && *b) {
        auto ca = static_cast<unsigned char>(*a++);
        auto cb = static_cast<unsigned char>(*b++);
        if (std::tolower(ca) != std::tolower(cb)) return false;
    }
    return *a == '\0' && *b == '\0';
}

inline bool ParseIpv4(const char* text, unsigned char out[4]) noexcept {
    if (!text || !*text) return false;
    const char* p = text;
    for (int part = 0; part < 4; ++part) {
        if (*p < '0' || *p > '9') return false;
        unsigned value = 0;
        int digits = 0;
        while (*p >= '0' && *p <= '9') {
            value = value * 10 + static_cast<unsigned>(*p - '0');
            if (++digits > 3 || value > 255) return false;
            ++p;
        }
        out[part] = static_cast<unsigned char>(value);
        if (part < 3) {
            if (*p != '.') return false;
            ++p;
        }
    }
    return *p == '\0';
}

inline bool IsLoopbackIpv4Bytes(const unsigned char addr[4]) noexcept {
    return addr && addr[0] == 127;
}

inline bool IsLoopbackIpv6Bytes(const unsigned char addr[16]) noexcept {
    if (!addr) return false;
    bool v6Loopback = true;
    for (int i = 0; i < 15; ++i) {
        if (addr[i] != 0) { v6Loopback = false; break; }
    }
    if (v6Loopback && addr[15] == 1) return true;
    for (int i = 0; i < 10; ++i) {
        if (addr[i] != 0) return false;
    }
    return addr[10] == 0xff && addr[11] == 0xff && addr[12] == 127;
}

inline bool IsLoopbackHostA(const char* host) noexcept {
    // Null or empty node name resolves local bind/loopback interfaces, never external DNS.
    if (!host || !*host) return true;
    if (EqualsIgnoreCaseAscii(host, "localhost") || EqualsIgnoreCaseAscii(host, "localhost.")) return true;
    if (std::strcmp(host, "::1") == 0 || std::strcmp(host, "[::1]") == 0 ||
        std::strcmp(host, "0:0:0:0:0:0:0:1") == 0) return true;
    unsigned char ipv4[4]{};
    if (ParseIpv4(host, ipv4)) return IsLoopbackIpv4Bytes(ipv4);
    // IPv4-mapped IPv6 loopback, e.g. "::ffff:127.0.0.1" or "[::ffff:127.0.0.1]".
    const char* p = host;
    char trimmed[64]{};
    size_t len = std::strlen(host);
    if (len >= 2 && host[0] == '[' && host[len - 1] == ']' && len - 2 < sizeof(trimmed)) {
        std::memcpy(trimmed, host + 1, len - 2);
        trimmed[len - 2] = '\0';
        p = trimmed;
    }
    if (std::strlen(p) > 7) {
        char prefix[8]{};
        std::memcpy(prefix, p, 7);
        if (EqualsIgnoreCaseAscii(prefix, "::ffff:") && ParseIpv4(p + 7, ipv4)) {
            return IsLoopbackIpv4Bytes(ipv4);
        }
    }
    return false;
}

inline bool IsLoopbackHostW(const wchar_t* host) noexcept {
    if (!host || !*host) return true;
    char ascii[65]{};
    size_t i = 0;
    for (; host[i] != L'\0'; ++i) {
        if (i >= 64 || host[i] > 0x7f) return false;
        ascii[i] = static_cast<char>(host[i]);
    }
    ascii[i] = '\0';
    return IsLoopbackHostA(ascii);
}

} // namespace tower_update_block
