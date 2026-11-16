#include "gate.h"
#include <cassert>
#include <iostream>
#include <map>
#include <vector>
#include <algorithm>

std::string NewEpoch() { return "0123456789abcdef0123456789abcdef"; }
void GateLog(const char*) {}
CSteamID Player(unsigned id) { return CSteamID(id, k_EUniversePublic, k_EAccountTypeIndividual); }
CSteamID Room(unsigned id) { return CSteamID(id, k_EUniversePublic, k_EAccountTypeChat); }
struct World {
    CSteamID room = Room(123), owner = Player(1);
    std::vector<CSteamID> members{Player(1)};
    std::map<std::string, std::string> data;
    std::map<uint64, std::map<std::string, std::string>> meta;
};
struct MockMatch : ForwardISteamMatchmaking {
    World& world; CSteamID self; int joins = 0, filters = 0;
    bool deferJoin = false, rejectJoin = false, nullData = false;
    MockMatch(World& w, CSteamID id): ForwardISteamMatchmaking(nullptr), world(w), self(id) {}
    bool joined() { return std::find(world.members.begin(), world.members.end(), self) != world.members.end(); }
    CSteamID GetLobbyOwner(CSteamID) override { return joined() ? world.owner : CSteamID(); }
    int GetNumLobbyMembers(CSteamID) override { return joined() ? int(world.members.size()) : 0; }
    CSteamID GetLobbyMemberByIndex(CSteamID, int i) override { return world.members.at(i); }
    const char* GetLobbyData(CSteamID, const char* key) override { return nullData ? nullptr : world.data[key].c_str(); }
    bool SetLobbyData(CSteamID, const char* key, const char* value) override { world.data[key] = value; return true; }
    const char* GetLobbyMemberData(CSteamID, CSteamID id, const char* key) override { return joined() ? world.meta[id.ConvertToUint64()][key].c_str() : nullptr; }
    void SetLobbyMemberData(CSteamID, const char* key, const char* value) override { world.meta[self.ConvertToUint64()][key] = value; }
    SteamAPICall_t CreateLobby(ELobbyType, int) override { return 10; }
    SteamAPICall_t JoinLobby(CSteamID) override { joins++; if(rejectJoin) return 0; if(!deferJoin) world.members.push_back(self); return 11; }
    void LeaveLobby(CSteamID) override {}
    bool SetLobbyJoinable(CSteamID, bool) override { return true; }
    void AddRequestLobbyListStringFilter(const char* key, const char* value, ELobbyComparison op) override {
        assert(!std::strcmp(key, RealmKey) && !std::strcmp(value, Realm) && op == k_ELobbyComparisonEqual); ++filters;
    }
    SteamAPICall_t RequestLobbyList() override { return 12; }
    CSteamID GetLobbyByIndex(int) override { return world.room; }
};
struct MockMessages : ForwardISteamNetworkingMessages {
    int sent = 0, accepted = 0;
    std::vector<SteamNetworkingMessage_t*> pending;
    MockMessages(): ForwardISteamNetworkingMessages(nullptr) {}
    EResult SendMessageToUser(const SteamNetworkingIdentity&, const void*, uint32, int, int) override { ++sent; return k_EResultOK; }
    bool AcceptSessionWithUser(const SteamNetworkingIdentity&) override { ++accepted; return true; }
    int ReceiveMessagesOnChannel(int, SteamNetworkingMessage_t** out, int maximum) override {
        int n = 0; for (auto m : pending) if (n < maximum) out[n++] = m; return n;
    }
};
int released = 0;
void ReleaseMessage(SteamNetworkingMessage_t*) { ++released; }
SteamNetworkingIdentity Identity(CSteamID id) { SteamNetworkingIdentity i; i.Clear(); i.SetSteamID(id); return i; }
#ifndef GATE_ABI_TEST
int main() {
    {
        World staged;
        MockMatch hostApi(staged, Player(1)), guestApi(staged, Player(2));
        Admission host(&hostApi, Player(1)), guest(&guestApi, Player(2));
        MatchGate hostMatch(&hostApi,host), guestMatch(&guestApi,guest);
        assert(hostMatch.CreateLobby(k_ELobbyTypePrivate,6));
        assert(hostMatch.SetLobbyData(staged.room,"test","1"));
        guestApi.nullData=true;
        assert(!guestMatch.JoinLobby(staged.room));
        guestApi.nullData=false;guestApi.rejectJoin=true;
        assert(!guestMatch.JoinLobby(staged.room) && !guest.lobby.IsValid());
        guestApi.rejectJoin=false;guestApi.deferJoin=true;
        assert(guestMatch.JoinLobby(staged.room));
        assert(!guest.allow(Player(1))); // Joined callback/data have not arrived yet.
        staged.members.push_back(Player(2));
        auto saved=staged.meta[Player(1).ConvertToUint64()][MemberKey];
        staged.meta[Player(1).ConvertToUint64()][MemberKey].clear();
        assert(!guest.allow(Player(1))); // Owner metadata has not arrived yet.
        staged.meta[Player(1).ConvertToUint64()][MemberKey]=saved;
        assert(guestMatch.GetNumLobbyMembers(staged.room)==2);
        assert(host.allow(Player(2)) && guest.allow(Player(1)));
        staged.members.pop_back();
        assert(!host.allow(Player(2)) && !guest.allow(Player(1)));
        guestMatch.LeaveLobby(staged.room);
        guestApi.deferJoin=false;
        assert(guestMatch.JoinLobby(staged.room) && guest.allow(Player(1)));
        assert(!guest.allow(Player(9))); // No roster membership, even if metadata is forged.
        guestApi.nullData=true;
        assert(!guest.allow(Player(1)));
        std::cout << "PASS: pre-join member privacy, asynchronous arrival, missing metadata, retry, disconnect, null data\n";
    }
    World world; MockMatch hostApi(world, Player(1)), guestApi(world, Player(2));
    Admission host(&hostApi, Player(1)), guest(&guestApi, Player(2));
    MatchGate hostMatch(&hostApi, host), guestMatch(&guestApi, guest);
    MockMessages transport; MessageGate net(&transport, host);
    auto peer = Identity(Player(2));
    assert(!net.AcceptSessionWithUser(peer));
    assert(net.SendMessageToUser(peer, "x", 1, 0, 0) == k_EResultAccessDenied);
    assert(!transport.sent && !transport.accepted);
    assert(!guestMatch.JoinLobby(world.room) && guestApi.joins == 0);
    assert(guestMatch.RequestLobbyList() == 12 && guestApi.filters == 1);
    assert(!guestMatch.GetLobbyByIndex(0).IsValid());
    assert(hostMatch.CreateLobby(k_ELobbyTypePublic, 6) == 10);
    assert(hostMatch.SetLobbyData(world.room, "password", "same-password"));
    assert(guestMatch.GetLobbyByIndex(0) == world.room);
    assert(guestMatch.JoinLobby(world.room) == 11);
    assert(!host.allow(Player(2))); // Joining the lobby alone is not admission.
    assert(guest.allow(Player(1))); // Guest publishes its own member attestation.
    assert(host.allow(Player(2)));
    assert(net.AcceptSessionWithUser(peer));
    assert(net.SendMessageToUser(peer, "x", 1, 0, 0) == k_EResultOK);
    world.members.push_back(Player(3)); // Unmodified client with identical password.
    auto original = Identity(Player(3));
    assert(!net.AcceptSessionWithUser(original));
    assert(net.SendMessageToUser(original, "x", 1, 0, 0) == k_EResultAccessDenied);
    assert(transport.sent == 1 && transport.accepted == 1);
    struct TestMessage : SteamNetworkingMessage_t { TestMessage() { std::memset(this, 0, sizeof(*this)); } ~TestMessage() = default; };
    TestMessage good{}, bad{};
    good.m_identityPeer = peer; bad.m_identityPeer = original;
    good.m_pfnRelease = ReleaseMessage; bad.m_pfnRelease = ReleaseMessage;
    transport.pending = {&bad, &good}; SteamNetworkingMessage_t* out[2]{};
    assert(net.ReceiveMessagesOnChannel(0, out, 2) == 1 && out[0] == &good && out[1] == nullptr && released == 1);
    world.meta[Player(2).ConvertToUint64()][MemberKey] = "old-session";
    assert(!host.allow(Player(2)));
    world.meta[Player(2).ConvertToUint64()][MemberKey] = host.epoch;
    world.data[EpochKey] = "ffffffffffffffffffffffffffffffff";
    assert(!host.allow(Player(2)) && !guest.allow(Player(1)));
    world.data[EpochKey] = host.epoch;
    world.owner = Player(3); assert(!host.allow(Player(2)));
    world.owner = Player(1);
    assert(!hostMatch.SetLobbyData(world.room, RealmKey, "vanilla"));
    assert(!hostMatch.DeleteLobbyData(world.room, EpochKey));
    hostMatch.LeaveLobby(world.room); assert(!host.allow(Player(2)));
    std::cout << "PASS: namespace, direct joins, reciprocal membership, implicit send acceptance, receive filtering, stale epoch, owner change, leave revocation\n";
}
#endif
