#include <windows.h>
#include <intrin.h>
#include <cstring>
#include "MinHook.h"

using Resolver = void* (*)(int, const char*);
extern "C" void* TowerFindInterface(int, const char*, Resolver) noexcept;
void GateLog(const char* event);
static Resolver original = nullptr;

static void* FindInterface(int user, const char* version) {
    HMODULE caller = nullptr;
    if (version && GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                                     reinterpret_cast<LPCWSTR>(_ReturnAddress()), &caller) &&
        caller == GetModuleHandleW(L"ersc.dll") &&
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
