#include "gate.h"
#include <windows.h>
#include <bcrypt.h>
#include <memory>
#include <fstream>
#include <map>
#pragma comment(lib, "bcrypt.lib")

std::string NewEpoch() {
    unsigned char bytes[16];
    if (BCryptGenRandom(nullptr, bytes, sizeof(bytes), BCRYPT_USE_SYSTEM_PREFERRED_RNG) < 0) return {};
    const char* hex = "0123456789abcdef";
    std::string value;
    for (auto byte : bytes) { value += hex[byte >> 4]; value += hex[byte & 15]; }
    return value;
}

void GateLog(const char* event) {
    try {
        static std::mutex logMutex;
        std::lock_guard<std::mutex> lock(logMutex);
        static std::map<std::string, ULONGLONG> last;
        auto now = GetTickCount64();
        auto found = last.find(event);
        if (found != last.end() && now - found->second < 5000) return;
        last[event] = now;
        wchar_t path[32768];
        HMODULE module = nullptr;
        if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                               reinterpret_cast<LPCWSTR>(&GateLog), &module)) return;
        DWORD count = GetModuleFileNameW(module, path, 32768);
        if (!count || count >= 32768) return;
        std::wstring file(path, count);
        file.resize(file.find_last_of(L"\\/") + 1);
        file += L"tower-session-gate.log";
        std::ofstream stream(file, std::ios::app);
        stream << GetTickCount64() << " " << event << '\n';
    } catch (...) { }
}

using Resolver = void* (*)(int, const char*);
extern "C" __declspec(dllexport) void* TowerFindInterface(int user, const char* version, Resolver resolve) noexcept {
    try {
        static std::mutex mutex;
        static int boundUser = 0;
        static Resolver boundResolver = nullptr;
        static std::unique_ptr<Admission> admission;
        static std::unique_ptr<MatchGate> matchmaking;
        static std::unique_ptr<MessageGate> messages;
        static std::unique_ptr<LegacyGate> legacy;
        std::lock_guard<std::mutex> lock(mutex);
        if (!version || !resolve || !user) return nullptr;
        if (!admission) {
            auto mm = static_cast<ISteamMatchmaking*>(resolve(user, STEAMMATCHMAKING_INTERFACE_VERSION));
            auto self = static_cast<ISteamUser*>(resolve(user, STEAMUSER_INTERFACE_VERSION));
            if (!mm || !self || !self->GetSteamID().IsValid()) { GateLog("interface initialization denied"); return nullptr; }
            admission.reset(new Admission(mm, self->GetSteamID()));
            matchmaking.reset(new MatchGate(mm, *admission));
            boundUser = user; boundResolver = resolve;
            GateLog("private session adapter initialized; protocol v1");
        }
        if (boundUser != user || boundResolver != resolve) return nullptr;
        if (!std::strcmp(version, STEAMMATCHMAKING_INTERFACE_VERSION)) { GateLog("matchmaking interface acquired"); return matchmaking.get(); }
        if (!std::strcmp(version, STEAMNETWORKINGMESSAGES_INTERFACE_VERSION)) {
            if (!messages) {
                auto real = static_cast<ISteamNetworkingMessages*>(resolve(user, version));
                if (!real) return nullptr;
                messages.reset(new MessageGate(real, *admission));
                GateLog("messages interface acquired");
            }
            return messages.get();
        }
        if (!std::strcmp(version, STEAMNETWORKING_INTERFACE_VERSION)) {
            if (!legacy) {
                auto real = static_cast<ISteamNetworking*>(resolve(user, version));
                if (!real) return nullptr;
                legacy.reset(new LegacyGate(real, *admission));
                GateLog("legacy network interface acquired");
            }
            return legacy.get();
        }
        GateLog("unknown interface rejected");
        return nullptr;
    } catch (...) { GateLog("interface initialization failed"); return nullptr; }
}
