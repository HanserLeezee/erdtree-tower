#include "gate.h"
#include <windows.h>
#include <iostream>
#include <stdexcept>
#include <vector>
using Resolver = void* (*)(int, const char*);
using Pump = void (*)();
void require(bool condition, const char* message) { if (!condition) throw std::runtime_error(message); }
template<class T> T result(ISteamUtils* utils, Pump pump, SteamAPICall_t call) {
    require(call != 0, "Steam call rejected");
    bool failed = false;
    auto start = GetTickCount64();
    while (!utils->IsAPICallCompleted(call, &failed)) {
        require(GetTickCount64() - start < 25000, "Steam call timed out"); pump(); Sleep(50);
    }
    T value{};
    require(!failed && utils->GetAPICallResult(call, &value, sizeof(value), T::k_iCallback, &failed) && !failed, "Steam call result failed");
    return value;
}
int main(int argc, char** argv) {
    if (argc != 2) return 2;
    SetEnvironmentVariableA("SteamAppId", "1245620");
    auto api = LoadLibraryA(argv[1]);
    require(api != nullptr, "Original Steam API unavailable");
    auto init = reinterpret_cast<bool(*)()>(GetProcAddress(api,"SteamAPI_Init"));
    auto pump = reinterpret_cast<Pump>(GetProcAddress(api,"SteamAPI_RunCallbacks"));
    auto shutdown = reinterpret_cast<Pump>(GetProcAddress(api,"SteamAPI_Shutdown"));
    auto handle = reinterpret_cast<int(*)()>(GetProcAddress(api,"SteamAPI_GetHSteamUser"));
    auto utilsFn = reinterpret_cast<ISteamUtils*(*)()>(GetProcAddress(api,"SteamAPI_SteamUtils_v010"));
    auto resolve = reinterpret_cast<Resolver>(GetProcAddress(api,"SteamInternal_FindOrCreateUserInterface"));
    require(init && pump && shutdown && handle && utilsFn && resolve && init(), "Steam initialization failed");
    auto utils = utilsFn();
    auto raw = static_cast<ISteamMatchmaking*>(resolve(handle(),STEAMMATCHMAKING_INTERFACE_VERSION));
    std::vector<CSteamID> rooms;
    int exit = 0;
    try {
        auto adapter = LoadLibraryW(L".\\Tower.SeamlessGate.dll"); require(adapter != nullptr,"Adapter load failed");
        using Wrap = void*(*)(int,const char*,Resolver);
        auto wrap = reinterpret_cast<Wrap>(GetProcAddress(adapter,"TowerFindInterface")); require(wrap != nullptr,"Adapter export missing");
        auto mm = static_cast<ISteamMatchmaking*>(wrap(handle(),STEAMMATCHMAKING_INTERFACE_VERSION,resolve)); require(mm != nullptr,"Adapter initialization failed");
        for(int cycle = 0; cycle < 2; ++cycle) {
            auto created = result<LobbyCreated_t>(utils,pump,mm->CreateLobby(k_ELobbyTypePrivate,6));
            require(created.m_eResult == k_EResultOK,"Private lobby creation failed");
            CSteamID room(created.m_ulSteamIDLobby); rooms.push_back(room);
            require(mm->SetLobbyData(room,"local_test","1"),"Namespace publication failed");
            require(std::strcmp(SafeText(raw->GetLobbyData(room,RealmKey)),Realm)==0,"Namespace missing from real Steam lobby");
            auto owner=raw->GetLobbyOwner(room);
            std::string epoch=SafeText(raw->GetLobbyData(room,EpochKey));
            require(epoch.size()==32 && epoch==SafeText(raw->GetLobbyMemberData(room,owner,MemberKey)),"Real Steam member attestation unavailable");
            mm->LeaveLobby(room); rooms.pop_back();
            std::cout << "PASS: real Steam private lobby create, namespace/member publication, leave (cycle " << cycle+1 << ")\n";
        }
        auto created=result<LobbyCreated_t>(utils,pump,raw->CreateLobby(k_ELobbyTypePrivate,6));
        require(created.m_eResult==k_EResultOK,"Control lobby creation failed");
        CSteamID room(created.m_ulSteamIDLobby);rooms.push_back(room);
        require(mm->JoinLobby(room)==0,"Untagged control lobby was accepted");
        std::cout << "PASS: real Steam untagged lobby rejected before JoinLobby\n";
    } catch(const std::exception& e) { std::cerr << "FAIL: " << e.what() << '\n';exit=1; }
    for(auto room:rooms) raw->LeaveLobby(room);
    shutdown();
    return exit;
}
