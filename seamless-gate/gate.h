#pragma once
#include "forwarders.h"
#include <cstring>
#include <mutex>
#include <string>
#include <vector>
#include <algorithm>

inline constexpr char RealmKey[] = "tower_gate";
inline constexpr char EpochKey[] = "tower_epoch";
inline constexpr char MemberKey[] = "tower_member";
inline constexpr char Realm[] = "erdtree-tower-private-v1";
std::string NewEpoch();
void GateLog(const char* event);
inline const char* SafeText(const char* value) { return value ? value : ""; }

struct Admission {
    ISteamMatchmaking* mm;
    CSteamID local, lobby, owner;
    std::string epoch;
    bool creating = false;
    std::recursive_mutex mutex;

    Admission(ISteamMatchmaking* matchmaking, CSteamID self): mm(matchmaking), local(self) {}
    void clear() { lobby = CSteamID(); owner = CSteamID(); epoch.clear(); creating = false; }
    bool member(CSteamID room, CSteamID peer) {
        int n = mm->GetNumLobbyMembers(room);
        if (n < 1 || n > 250) return false;
        for (int i = 0; i < n; ++i) if (mm->GetLobbyMemberByIndex(room, i) == peer) return true;
        return false;
    }
    bool realm(CSteamID room) {
        return room.IsValid() && std::strcmp(SafeText(mm->GetLobbyData(room, RealmKey)), Realm) == 0;
    }
    bool adopt(CSteamID room) {
        if (!realm(room)) return false;
        std::string value = SafeText(mm->GetLobbyData(room, EpochKey));
        if (value.size() != 32 || value.find_first_not_of("0123456789abcdef") != std::string::npos) return false;
        // Steam exposes member information only after joining. Admission stays closed until valid().
        lobby = room; owner = CSteamID(); epoch = value; creating = false;
        return true;
    }
    bool publish(CSteamID room) {
        if (room == lobby && !epoch.empty()) return valid();
        if (!creating || mm->GetLobbyOwner(room) != local || !member(room, local)) return false;
        auto value = NewEpoch();
        if (value.size() != 32) return false;
        // Namespace is published last; incomplete rooms cannot pass admission.
        if (!mm->SetLobbyData(room, EpochKey, value.c_str())) return false;
        mm->SetLobbyMemberData(room, MemberKey, value.c_str());
        if (!mm->SetLobbyData(room, RealmKey, Realm)) return false;
        lobby = room; owner = local; epoch = value; creating = false;
        GateLog("host namespace published");
        return true;
    }
    bool valid(CSteamID peer = CSteamID()) {
        if (epoch.empty() || !realm(lobby) || epoch != SafeText(mm->GetLobbyData(lobby, EpochKey))) return false;
        auto host = mm->GetLobbyOwner(lobby);
        if (!host.IsValid() || (owner.IsValid() && host != owner)) return false;
        bool hasLocal = false, hasHost = false, hasPeer = !peer.IsValid();
        int count = mm->GetNumLobbyMembers(lobby);
        if (count < 1 || count > 250) return false;
        for (int i = 0; i < count && !(hasLocal && hasHost && hasPeer); ++i) {
            auto id = mm->GetLobbyMemberByIndex(lobby, i);
            hasLocal |= id == local; hasHost |= id == host; hasPeer |= id == peer;
        }
        if (!hasLocal || !hasHost || !hasPeer ||
            epoch != SafeText(mm->GetLobbyMemberData(lobby, host, MemberKey))) return false;
        owner = host;
        if (epoch != SafeText(mm->GetLobbyMemberData(lobby, local, MemberKey)))
            mm->SetLobbyMemberData(lobby, MemberKey, epoch.c_str());
        return !peer.IsValid() || epoch == SafeText(mm->GetLobbyMemberData(lobby, peer, MemberKey));
    }
    bool allow(CSteamID peer) {
        std::lock_guard<std::recursive_mutex> lock(mutex);
        return peer.IsValid() && peer != local && valid(peer);
    }
};

class MatchGate final : public ForwardISteamMatchmaking {
    Admission& gate;
public:
    MatchGate(ISteamMatchmaking* real, Admission& state): ForwardISteamMatchmaking(real), gate(state) {}
    SteamAPICall_t RequestLobbyList() override {
        real->AddRequestLobbyListStringFilter(RealmKey, Realm, k_ELobbyComparisonEqual);
        return real->RequestLobbyList();
    }
    CSteamID GetLobbyByIndex(int index) override {
        auto room = real->GetLobbyByIndex(index);
        return gate.realm(room) ? room : CSteamID();
    }
    SteamAPICall_t CreateLobby(ELobbyType type, int count) override {
        std::lock_guard<std::recursive_mutex> lock(gate.mutex);
        if (gate.lobby.IsValid() || gate.creating || count < 1 || count > 250) return 0;
        gate.creating = true;
        GateLog("host creation requested");
        auto call = real->CreateLobby(type, count);
        if (!call) gate.creating = false;
        return call;
    }
    SteamAPICall_t JoinLobby(CSteamID room) override {
        std::lock_guard<std::recursive_mutex> lock(gate.mutex);
        if (gate.lobby.IsValid() || gate.creating || !gate.adopt(room)) {
            GateLog("join rejected: foreign or incomplete session"); return 0;
        }
        auto call = real->JoinLobby(room);
        GateLog("private session join requested");
        if (!call) gate.clear();
        return call;
    }
    void LeaveLobby(CSteamID room) override {
        std::lock_guard<std::recursive_mutex> lock(gate.mutex);
        if (gate.lobby == room || gate.creating) gate.clear();
        real->LeaveLobby(room);
    }
    int GetNumLobbyMembers(CSteamID room) override {
        std::lock_guard<std::recursive_mutex> lock(gate.mutex);
        if (room == gate.lobby) gate.valid();
        return real->GetNumLobbyMembers(room);
    }
    const char* GetLobbyData(CSteamID room, const char* key) override {
        std::lock_guard<std::recursive_mutex> lock(gate.mutex);
        if (room == gate.lobby) gate.valid();
        return real->GetLobbyData(room, key);
    }
    bool SetLobbyData(CSteamID room, const char* key, const char* value) override {
        std::lock_guard<std::recursive_mutex> lock(gate.mutex);
        if (!std::strcmp(key, RealmKey) || !std::strcmp(key, EpochKey)) return false;
        return gate.publish(room) && real->SetLobbyData(room, key, value);
    }
    bool DeleteLobbyData(CSteamID room, const char* key) override {
        if (!std::strcmp(key, RealmKey) || !std::strcmp(key, EpochKey)) return false;
        return real->DeleteLobbyData(room, key);
    }
    bool SetLobbyJoinable(CSteamID room, bool joinable) override {
        std::lock_guard<std::recursive_mutex> lock(gate.mutex);
        return (!joinable || gate.publish(room)) && real->SetLobbyJoinable(room, joinable);
    }
    bool SetLobbyOwner(CSteamID, CSteamID) override { return false; }
    bool SetLinkedLobby(CSteamID, CSteamID) override { return false; }
    bool SendLobbyChatMsg(CSteamID, const void*, int) override { return false; }
    int GetLobbyChatEntry(CSteamID, int, CSteamID*, void*, int, EChatEntryType*) override { return 0; }
    void SetLobbyMemberData(CSteamID room, const char* key, const char* value) override {
        if (std::strcmp(key, MemberKey)) real->SetLobbyMemberData(room, key, value);
    }
};

class MessageGate final : public ForwardISteamNetworkingMessages {
    Admission& gate;
public:
    MessageGate(ISteamNetworkingMessages* real, Admission& state): ForwardISteamNetworkingMessages(real), gate(state) {}
    EResult SendMessageToUser(const SteamNetworkingIdentity& peer, const void* data, uint32 size, int flags, int channel) override {
        if (!gate.allow(peer.GetSteamID())) { GateLog("message send denied: peer not admitted"); return k_EResultAccessDenied; }
        return real->SendMessageToUser(peer, data, size, flags, channel);
    }
    bool AcceptSessionWithUser(const SteamNetworkingIdentity& peer) override {
        if (!gate.allow(peer.GetSteamID())) { GateLog("message connection denied: peer not admitted"); return false; }
        return real->AcceptSessionWithUser(peer);
    }
    int ReceiveMessagesOnChannel(int channel, SteamNetworkingMessage_t** out, int maximum) override {
        if (!out || maximum <= 0) return 0;
        int count = real->ReceiveMessagesOnChannel(channel, out, maximum), kept = 0;
        if (count <= 0) return 0;
        // Only reuse admission within this batch. The next API call rechecks live state.
        std::vector<std::pair<CSteamID, bool>> checked;
        std::lock_guard<std::recursive_mutex> lock(gate.mutex);
        for (int i = 0; i < count; ++i) {
            auto message = out[i];
            bool allowed = false;
            if (message) {
                auto peer = message->m_identityPeer.GetSteamID();
                auto found = std::find_if(checked.begin(), checked.end(), [peer](const auto& entry) { return entry.first == peer; });
                if (found == checked.end()) { allowed = gate.allow(peer); checked.emplace_back(peer, allowed); }
                else allowed = found->second;
            }
            if (message && allowed) out[kept++] = message;
            else if (message) message->Release();
        }
        for (int i = kept; i < count; ++i) out[i] = nullptr;
        return kept;
    }
};

class LegacyGate final : public ForwardISteamNetworking {
    Admission& gate;
public:
    LegacyGate(ISteamNetworking* real, Admission& state): ForwardISteamNetworking(real), gate(state) {}
    bool SendP2PPacket(CSteamID peer, const void* data, uint32 size, EP2PSend type, int channel) override {
        return gate.allow(peer) && real->SendP2PPacket(peer, data, size, type, channel);
    }
    bool AcceptP2PSessionWithUser(CSteamID peer) override {
        return gate.allow(peer) && real->AcceptP2PSessionWithUser(peer);
    }
    bool ReadP2PPacket(void* out, uint32 capacity, uint32* size, CSteamID* peer, int channel) override {
        if (!out || !size || !peer) return false;
        if (!real->ReadP2PPacket(out, capacity, size, peer, channel)) return false;
        if (gate.allow(*peer)) return true;
        std::memset(out, 0, capacity); *size = 0; *peer = CSteamID(); return false;
    }
    SNetListenSocket_t CreateListenSocket(int, SteamIPAddress_t, uint16, bool) override { return 0; }
    SNetSocket_t CreateP2PConnectionSocket(CSteamID, int, int, bool) override { return 0; }
    SNetSocket_t CreateConnectionSocket(SteamIPAddress_t, uint16, int) override { return 0; }
    bool SendDataOnSocket(SNetSocket_t, void*, uint32, bool) override { return false; }
    bool RetrieveDataFromSocket(SNetSocket_t, void*, uint32, uint32*) override { return false; }
    bool RetrieveData(SNetListenSocket_t, void*, uint32, uint32*, SNetSocket_t*) override { return false; }
};
