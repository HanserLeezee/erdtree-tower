#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>
#include <intrin.h>
#include <cstring>
#include "MinHook.h"
#include "update_block.h"

using Resolver = void* (*)(int, const char*);
using GetAddrInfoAFn = int (WSAAPI*)(PCSTR, PCSTR, const ADDRINFOA*, PADDRINFOA*);
using GetAddrInfoWFn = int (WSAAPI*)(PCWSTR, PCWSTR, const ADDRINFOW*, PADDRINFOW*);
using GetAddrInfoExWFn = int (WSAAPI*)(PCWSTR, PCWSTR, DWORD, LPGUID, const ADDRINFOEXW*, PADDRINFOEXW*,
                                       timeval*, LPOVERLAPPED, LPLOOKUPSERVICE_COMPLETION_ROUTINE, LPHANDLE);
using GetHostByNameFn = hostent* (WSAAPI*)(const char*);
using ConnectFn = int (WSAAPI*)(SOCKET, const sockaddr*, int);
using WSAConnectFn = int (WSAAPI*)(SOCKET, const sockaddr*, int, LPWSABUF, LPWSABUF, LPQOS, LPQOS);

extern "C" void* TowerFindInterface(int, const char*, Resolver) noexcept;
void GateLog(const char* event);
static Resolver original = nullptr;
static GetAddrInfoAFn originalGetAddrInfoA = nullptr;
static GetAddrInfoWFn originalGetAddrInfoW = nullptr;
static GetAddrInfoExWFn originalGetAddrInfoExW = nullptr;
static GetHostByNameFn originalGetHostByName = nullptr;
static ConnectFn originalConnect = nullptr;
static WSAConnectFn originalWSAConnect = nullptr;

static bool IsErscCaller(void* returnAddress) noexcept {
    HMODULE ersc = GetModuleHandleW(L"ersc.dll");
    if (!ersc) return false;
    HMODULE caller = nullptr;
    if (GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                           reinterpret_cast<LPCWSTR>(returnAddress), &caller)) {
        return caller == ersc;
    }
    // WinLicense may bounce IAT calls through unbacked VirtualAlloc thunks; walk to the first PE module.
    void* frames[6]{};
    USHORT count = RtlCaptureStackBackTrace(1, 6, frames, nullptr);
    for (USHORT i = 0; i < count; ++i) {
        HMODULE frameModule = nullptr;
        if (GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                               reinterpret_cast<LPCWSTR>(frames[i]), &frameModule)) {
            return frameModule == ersc;
        }
    }
    return false;
}

static bool IsLoopbackSockAddr(const sockaddr* name, int namelen) noexcept {
    if (!name || namelen < static_cast<int>(sizeof(ADDRESS_FAMILY))) return false;
    if (name->sa_family == AF_INET) {
        if (namelen < static_cast<int>(sizeof(sockaddr_in))) return false;
        const auto* v4 = reinterpret_cast<const sockaddr_in*>(name);
        unsigned char bytes[4]{};
        std::memcpy(bytes, &v4->sin_addr, 4);
        return tower_update_block::IsLoopbackIpv4Bytes(bytes);
    }
    if (name->sa_family == AF_INET6) {
        if (namelen < static_cast<int>(sizeof(sockaddr_in6))) return false;
        const auto* v6 = reinterpret_cast<const sockaddr_in6*>(name);
        return tower_update_block::IsLoopbackIpv6Bytes(v6->sin6_addr.u.Byte);
    }
    // Non-IP socket families (e.g. local IPC) are unrelated to external HTTP update checks.
    return true;
}

static int WSAAPI HookGetAddrInfoA(PCSTR nodeName, PCSTR serviceName, const ADDRINFOA* hints, PADDRINFOA* result) {
    if (IsErscCaller(_ReturnAddress()) && !tower_update_block::IsLoopbackHostA(nodeName)) {
        if (result) *result = nullptr;
        GateLog("blocked ERSC external DNS lookup (getaddrinfo)");
        WSASetLastError(WSAHOST_NOT_FOUND);
        return WSAHOST_NOT_FOUND;
    }
    return originalGetAddrInfoA(nodeName, serviceName, hints, result);
}

static int WSAAPI HookGetAddrInfoW(PCWSTR nodeName, PCWSTR serviceName, const ADDRINFOW* hints, PADDRINFOW* result) {
    if (IsErscCaller(_ReturnAddress()) && !tower_update_block::IsLoopbackHostW(nodeName)) {
        if (result) *result = nullptr;
        GateLog("blocked ERSC external DNS lookup (GetAddrInfoW)");
        WSASetLastError(WSAHOST_NOT_FOUND);
        return WSAHOST_NOT_FOUND;
    }
    return originalGetAddrInfoW(nodeName, serviceName, hints, result);
}

static int WSAAPI HookGetAddrInfoExW(PCWSTR name, PCWSTR serviceName, DWORD nameSpace, LPGUID nspId,
                                     const ADDRINFOEXW* hints, PADDRINFOEXW* result, timeval* timeout,
                                     LPOVERLAPPED overlapped, LPLOOKUPSERVICE_COMPLETION_ROUTINE completionRoutine,
                                     LPHANDLE handle) {
    if (IsErscCaller(_ReturnAddress()) && !tower_update_block::IsLoopbackHostW(name)) {
        if (result) *result = nullptr;
        GateLog("blocked ERSC external DNS lookup (GetAddrInfoExW)");
        WSASetLastError(WSAHOST_NOT_FOUND);
        return WSAHOST_NOT_FOUND;
    }
    return originalGetAddrInfoExW(name, serviceName, nameSpace, nspId, hints, result, timeout,
                                  overlapped, completionRoutine, handle);
}

static hostent* WSAAPI HookGetHostByName(const char* name) {
    if (IsErscCaller(_ReturnAddress()) && !tower_update_block::IsLoopbackHostA(name)) {
        GateLog("blocked ERSC external DNS lookup (gethostbyname)");
        WSASetLastError(WSAHOST_NOT_FOUND);
        return nullptr;
    }
    return originalGetHostByName(name);
}

static int WSAAPI HookConnect(SOCKET s, const sockaddr* name, int namelen) {
    if (IsErscCaller(_ReturnAddress()) && !IsLoopbackSockAddr(name, namelen)) {
        GateLog("blocked ERSC external socket connect");
        WSASetLastError(WSAECONNREFUSED);
        return SOCKET_ERROR;
    }
    return originalConnect(s, name, namelen);
}

static int WSAAPI HookWSAConnect(SOCKET s, const sockaddr* name, int namelen, LPWSABUF callerData,
                                 LPWSABUF calleeData, LPQOS sQos, LPQOS gQos) {
    if (IsErscCaller(_ReturnAddress()) && !IsLoopbackSockAddr(name, namelen)) {
        GateLog("blocked ERSC external socket WSAConnect");
        WSASetLastError(WSAECONNREFUSED);
        return SOCKET_ERROR;
    }
    return originalWSAConnect(s, name, namelen, callerData, calleeData, sQos, gQos);
}

template <typename Fn>
static bool InstallOptionalHook(HMODULE module, const char* name, Fn detour, Fn* targetOut) noexcept {
    if (!module || *targetOut) return *targetOut != nullptr;
    auto address = GetProcAddress(module, name);
    if (!address) return false;
    return MH_CreateHook(reinterpret_cast<void*>(address), reinterpret_cast<void*>(detour),
                         reinterpret_cast<void**>(targetOut)) == MH_OK &&
           MH_EnableHook(reinterpret_cast<void*>(address)) == MH_OK;
}

static void InstallUpdateBlocker() noexcept {
    HMODULE ws2 = GetModuleHandleW(L"ws2_32.dll");
    if (!ws2) ws2 = LoadLibraryW(L"ws2_32.dll");
    if (!ws2) {
        GateLog("WARN: ws2_32.dll unavailable; ERSC update blocker skipped");
        return;
    }
    int installed = 0;
    installed += InstallOptionalHook(ws2, "getaddrinfo", &HookGetAddrInfoA, &originalGetAddrInfoA) ? 1 : 0;
    installed += InstallOptionalHook(ws2, "GetAddrInfoW", &HookGetAddrInfoW, &originalGetAddrInfoW) ? 1 : 0;
    installed += InstallOptionalHook(ws2, "GetAddrInfoExW", &HookGetAddrInfoExW, &originalGetAddrInfoExW) ? 1 : 0;
    installed += InstallOptionalHook(ws2, "gethostbyname", &HookGetHostByName, &originalGetHostByName) ? 1 : 0;
    installed += InstallOptionalHook(ws2, "connect", &HookConnect, &originalConnect) ? 1 : 0;
    installed += InstallOptionalHook(ws2, "WSAConnect", &HookWSAConnect, &originalWSAConnect) ? 1 : 0;
    if (installed > 0) {
        GateLog("ERSC external HTTP/DNS update check blocked before ERSC load");
    } else {
        GateLog("WARN: could not install ERSC Winsock update blocker hooks");
    }
}

static void* FindInterface(int user, const char* version) {
    if (version && IsErscCaller(_ReturnAddress()) &&
        (!std::strcmp(version, "SteamMatchMaking009") || !std::strcmp(version, "SteamNetworking006") ||
         !std::strcmp(version, "SteamNetworkingMessages002"))) {
        return TowerFindInterface(user, version, original);
    }
    return original(user, version);
}

extern "C" __declspec(dllexport) bool TowerGateInitialize() {
    if (original) return true;
    // Called outside DllMain, before loading ERSC. Never continue unguarded.
    if (!GetModuleHandleW(L"ersc.dll")) {
        auto steam = GetModuleHandleW(L"steam_api64.dll");
        auto address = steam ? GetProcAddress(steam, "SteamInternal_FindOrCreateUserInterface") : nullptr;
        if (address && MH_Initialize() == MH_OK &&
            MH_CreateHook(reinterpret_cast<void*>(address), reinterpret_cast<void*>(&FindInterface),
                          reinterpret_cast<void**>(&original)) == MH_OK &&
            MH_EnableHook(reinterpret_cast<void*>(address)) == MH_OK) {
            GateLog("Steam interface factory guarded before ERSC load; original ERSC file retained");
            InstallUpdateBlocker();
            return true;
        }
    }
    GateLog("FATAL: cannot install gate before ERSC; cancelling test startup");
    ExitProcess(ERROR_DLL_INIT_FAILED);
}

// ME3 runs this synchronously; a profile initializer moves ERSC onto a background thread.
extern "C" __declspec(dllexport) bool modengine_ext_init(void*, void** extension) {
    if (extension) *extension = nullptr;
    return TowerGateInitialize();
}
