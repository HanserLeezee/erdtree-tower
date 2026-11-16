#define GATE_ABI_TEST
#include "tests.cpp"
World world;
MockMatch match(world, Player(1));
MockMessages messages;
ForwardISteamNetworking legacy(nullptr);
class UserStub {
public:
    virtual HSteamUser GetHSteamUser() { return 1; }
    virtual bool BLoggedOn() { return true; }
    virtual CSteamID GetSteamID() { return Player(1); }
} user;
extern "C" __declspec(dllexport) __declspec(noinline) void* SteamInternal_FindOrCreateUserInterface(int id, const char* name) {
    if (id != 1) return nullptr;
    if (!std::strcmp(name, STEAMMATCHMAKING_INTERFACE_VERSION)) return &match;
    if (!std::strcmp(name, STEAMUSER_INTERFACE_VERSION)) return &user;
    if (!std::strcmp(name, STEAMNETWORKINGMESSAGES_INTERFACE_VERSION)) return &messages;
    if (!std::strcmp(name, STEAMNETWORKING_INTERFACE_VERSION)) return &legacy;
    return nullptr;
}
extern "C" __declspec(dllexport) void AddPeer(bool attested) {
    if (world.members.size() == 1) world.members.push_back(Player(2));
    if (attested) world.meta[Player(2).ConvertToUint64()][MemberKey] = world.data[EpochKey];
}
