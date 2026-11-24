#include <winsock2.h>
#include <ws2tcpip.h>
#include "gate.h"
#include <cassert>
#include <windows.h>
static bool loader_test = false;
extern "C" __declspec(dllimport) void* SteamInternal_FindOrCreateUserInterface(int, const char*);
extern "C" __declspec(dllimport) void AddPeer(bool);
extern "C" __declspec(dllexport) bool ExerciseGate() {
    auto mm = static_cast<ISteamMatchmaking*>(SteamInternal_FindOrCreateUserInterface(1, STEAMMATCHMAKING_INTERFACE_VERSION));
    auto room = CSteamID(123, k_EUniversePublic, k_EAccountTypeChat);
    assert(mm->CreateLobby(k_ELobbyTypePublic, 6));
    assert(mm->SetLobbyData(room, "password", "identical"));
    assert(std::strcmp(mm->GetLobbyData(room, RealmKey), Realm) == 0);
    // Exercise the first networking call after the old five-second profiler threshold, under loader lock.
    if (loader_test) Sleep(5200);
    auto net = static_cast<ISteamNetworkingMessages*>(SteamInternal_FindOrCreateUserInterface(1, STEAMNETWORKINGMESSAGES_INTERFACE_VERSION));
    SteamNetworkingIdentity peer; peer.Clear(); peer.SetSteamID(CSteamID(2, k_EUniversePublic, k_EAccountTypeIndividual));
    AddPeer(false);
    assert(!net->AcceptSessionWithUser(peer));
    assert(net->SendMessageToUser(peer, "x", 1, 0, 0) == k_EResultAccessDenied);
    AddPeer(true);
    assert(net->AcceptSessionWithUser(peer));
    assert(net->SendMessageToUser(peer, "x", 1, 0, 0) == k_EResultOK);
    mm->LeaveLobby(room);
    assert(!net->AcceptSessionWithUser(peer));
    return true;
}
extern "C" __declspec(dllexport) bool ExerciseUpdateBlock() {
    ADDRINFOA hintsA{}; hintsA.ai_flags = AI_NUMERICHOST; hintsA.ai_family = AF_INET;
    PADDRINFOA resA = reinterpret_cast<PADDRINFOA>(0x1);
    assert(getaddrinfo("192.0.2.1", "443", &hintsA, &resA) == WSAHOST_NOT_FOUND && resA == nullptr);
    assert(getaddrinfo("127.0.0.1", "443", &hintsA, &resA) == 0 && resA != nullptr);
    freeaddrinfo(resA);
    ADDRINFOW hintsW{}; hintsW.ai_flags = AI_NUMERICHOST; hintsW.ai_family = AF_INET;
    PADDRINFOW resW = reinterpret_cast<PADDRINFOW>(0x1);
    assert(GetAddrInfoW(L"192.0.2.1", L"443", &hintsW, &resW) == WSAHOST_NOT_FOUND && resW == nullptr);
    assert(GetAddrInfoW(L"127.0.0.1", L"443", &hintsW, &resW) == 0 && resW != nullptr);
    FreeAddrInfoW(resW);
    SOCKET s = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    assert(s != INVALID_SOCKET);
    sockaddr_in ext{}; ext.sin_family = AF_INET; ext.sin_port = htons(443);
    assert(inet_pton(AF_INET, "192.0.2.1", &ext.sin_addr) == 1);
    assert(connect(s, reinterpret_cast<const sockaddr*>(&ext), sizeof(ext)) == SOCKET_ERROR);
    assert(WSAGetLastError() == WSAECONNREFUSED);
    closesocket(s);
    return true;
}
BOOL WINAPI DllMain(HINSTANCE, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) {
        loader_test = true;
        bool ok = ExerciseGate();
        loader_test = false;
        return ok;
    }
    return TRUE;
}
