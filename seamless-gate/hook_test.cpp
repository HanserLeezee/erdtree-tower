#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>
#include <cassert>
#include <iostream>
int main() {
    WSADATA wsa{};
    assert(WSAStartup(MAKEWORD(2, 2), &wsa) == 0);
    auto steam = LoadLibraryW(L".\\steam_api64.dll"); assert(steam);
    using Resolve = void* (*)(int, const char*);
    auto resolve = reinterpret_cast<Resolve>(GetProcAddress(steam, "SteamInternal_FindOrCreateUserInterface")); assert(resolve);
    auto unwrapped = resolve(1, "SteamMatchMaking009"); assert(unwrapped);
    auto adapter = LoadLibraryW(L".\\Tower.SeamlessGate.dll"); assert(adapter);
    auto initialize = reinterpret_cast<bool(*)(void*, void**)>(GetProcAddress(adapter, "modengine_ext_init"));
    void* extension = nullptr;
    assert(initialize && initialize(nullptr, &extension));
    assert(extension == nullptr);
    assert(resolve(1, "SteamMatchMaking009") == unwrapped); // Non-ERSC callers are unchanged.
    ADDRINFOA numericHints{}; numericHints.ai_flags = AI_NUMERICHOST; numericHints.ai_family = AF_INET;
    PADDRINFOA externalInfo = nullptr;
    assert(getaddrinfo("192.0.2.1", "443", &numericHints, &externalInfo) == 0 && externalInfo != nullptr);
    freeaddrinfo(externalInfo);
    auto ersc = LoadLibraryW(L".\\ersc.dll"); assert(ersc);
    auto test = reinterpret_cast<bool(*)()>(GetProcAddress(ersc, "ExerciseGate")); assert(test && test());
    auto updateTest = reinterpret_cast<bool(*)()>(GetProcAddress(ersc, "ExerciseUpdateBlock")); assert(updateTest && updateTest());
    assert(resolve(1, "SteamMatchMaking009") == unwrapped);
    externalInfo = nullptr;
    assert(getaddrinfo("192.0.2.1", "443", &numericHints, &externalInfo) == 0 && externalInfo != nullptr);
    freeaddrinfo(externalInfo);
    WSACleanup();
    std::cout << "PASS: real factory detour, ERSC update-check blocking, and caller filtering; unmodified callers unaffected\n";
}
