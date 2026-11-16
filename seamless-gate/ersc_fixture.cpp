#include "gate.h"
#include <cassert>
extern "C" __declspec(dllimport) void* SteamInternal_FindOrCreateUserInterface(int, const char*);
extern "C" __declspec(dllimport) void AddPeer(bool);
extern "C" __declspec(dllexport) bool ExerciseGate() {
    auto mm = static_cast<ISteamMatchmaking*>(SteamInternal_FindOrCreateUserInterface(1, STEAMMATCHMAKING_INTERFACE_VERSION));
    auto room = CSteamID(123, k_EUniversePublic, k_EAccountTypeChat);
    assert(mm->CreateLobby(k_ELobbyTypePublic, 6));
    assert(mm->SetLobbyData(room, "password", "identical"));
    assert(std::strcmp(mm->GetLobbyData(room, RealmKey), Realm) == 0);
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
