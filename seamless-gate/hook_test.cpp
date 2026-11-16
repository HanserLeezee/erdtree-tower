#include <windows.h>
#include <cassert>
#include <iostream>
int main() {
    auto steam = LoadLibraryW(L".\\steam_api64.dll"); assert(steam);
    using Resolve = void* (*)(int, const char*);
    auto resolve = reinterpret_cast<Resolve>(GetProcAddress(steam, "SteamInternal_FindOrCreateUserInterface")); assert(resolve);
    auto unwrapped = resolve(1, "SteamMatchMaking009"); assert(unwrapped);
    auto adapter = LoadLibraryW(L".\\Tower.SeamlessGate.dll"); assert(adapter);
    auto initialize = reinterpret_cast<bool(*)()>(GetProcAddress(adapter, "TowerGateInitialize")); assert(initialize && initialize());
    assert(resolve(1, "SteamMatchMaking009") == unwrapped); // Non-ERSC callers are unchanged.
    auto ersc = LoadLibraryW(L".\\ersc.dll"); assert(ersc);
    auto test = reinterpret_cast<bool(*)()>(GetProcAddress(ersc, "ExerciseGate")); assert(test && test());
    assert(resolve(1, "SteamMatchMaking009") == unwrapped);
    std::cout << "PASS: real factory detour and caller filtering; ERSC-only gates, unmodified callers unaffected\n";
}
