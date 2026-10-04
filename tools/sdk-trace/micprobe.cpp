// Drive the OBSBOT SDK's Vox SE (TWS / TX) functions on a Tiny 3, for traffic
// recording. These methods are exported by libdev but not declared in its
// header, so they're bound by symbol name and called with the Device as the
// first argument (Itanium C++ ABI).
//
// micprobe info
// micprobe pair <1|2> <0|1>      micprobe clearpair <1|2>
// micprobe mute <1|2> <0|1>      micprobe gain <1|2> <n>
// micprobe key <0..3>            micprobe source <n>
// micprobe sound <0|1>           micprobe select <0|1>
// micprobe match | rematch        micprobe blepair <0|1> <n> | bleexit
//
// Product type defaults to the Tiny 3; set OBSBOT_PRODUCT=<ObsbotProductType
// number> to drive another model.
#include <dev/devs.hpp>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <thread>

using TX = Device::DevTXType;
#define SYM(name) __asm__(name)
extern "C++" {
int32_t twsInfo(Device *, Device::DevTWSInfo &) SYM("_ZN6Device17cameraGetTWSInfoRERNS_10DevTWSInfoE");
int32_t getAudioSelect(Device *, Device::AudioSelectAttr &) SYM("_ZN6Device21cameraGetAudioSelectRERNS_15AudioSelectAttrE");
int32_t setAudioSelect(Device *, Device::AudioSelectParam &) SYM("_ZN6Device20cameraSetAudioSelectERNS_16AudioSelectParamE");
int32_t getSelectedSource(Device *, unsigned char &) SYM("_ZN6Device29cameraGetSelectedAudioSourceRERh");
int32_t setSource(Device *, int) SYM("_ZN6Device21cameraSetAudioSourceREi");
int32_t setKey(Device *, Device::DevTWSKeyType) SYM("_ZN6Device20cameraSetTWSKeyTypeRENS_13DevTWSKeyTypeE");
int32_t setSound(Device *, Device::DevTWSSoundMode) SYM("_ZN6Device22cameraSetTWSSoundModeRENS_15DevTWSSoundModeE");
int32_t battLevel(Device *, TX, unsigned char &) SYM("_ZN6Device21cameraTXGetBattLevelRENS_9DevTXTypeERh");
int32_t charging(Device *, TX, unsigned char &) SYM("_ZN6Device30cameraTXGetBattChargingStatusRENS_9DevTXTypeERh");
int32_t getMute(Device *, TX, bool &) SYM("_ZN6Device21cameraTXGetAudioMuteRENS_9DevTXTypeERb");
int32_t setMute(Device *, TX, bool) SYM("_ZN6Device21cameraTXSetAudioMuteRENS_9DevTXTypeEb");
int32_t getGain(Device *, TX, int &) SYM("_ZN6Device21cameraTXGetAudioGainRENS_9DevTXTypeERi");
int32_t setGain(Device *, TX, int) SYM("_ZN6Device21cameraTXSetAudioGainRENS_9DevTXTypeEi");
int32_t getName(Device *, TX, std::string &) SYM("_ZN6Device22cameraTXGetDeviceNameRENS_9DevTXTypeERNSt7__cxx1112basic_stringIcSt11char_traitsIcESaIcEEE");
int32_t getVersion(Device *, TX, unsigned int &) SYM("_ZN6Device18cameraTXGetVersionENS_9DevTXTypeERj");
int32_t getSn(Device *, TX, std::string &) SYM("_ZN6Device13cameraTXGetSnENS_9DevTXTypeERNSt7__cxx1112basic_stringIcSt11char_traitsIcESaIcEEE");
int32_t setPair(Device *, TX, bool) SYM("_ZN6Device22cameraTXSetPairEnabledENS_9DevTXTypeEb");
int32_t clearPair(Device *, TX) SYM("_ZN6Device23cameraTXClearPairedInfoENS_9DevTXTypeE");
int32_t btMatch(Device *) SYM("_ZN6Device24cameraDevBluetoothMatchUEv");
int32_t btRematch(Device *) SYM("_ZN6Device26cameraDevBluetoothRematchUEv");
int32_t blePairing(Device *, bool, unsigned char) SYM("_ZN6Device19setBlePairingEnableEbh");
int32_t blePairingExit(Device *) SYM("_ZN6Device17setBlePairingExitEv");
}

static void mark(const std::string &s) { fprintf(stderr, "=== %s\n", s.c_str()); fflush(stderr); }
static void pause() { std::this_thread::sleep_for(std::chrono::milliseconds(400)); }
static void onChanged(std::string, bool, void *) {}
static ObsbotProductType product() {
    const char *p = getenv("OBSBOT_PRODUCT");
    return p ? (ObsbotProductType)atoi(p) : ObsbotProdTiny3;
}

int main(int argc, char **argv) {
    if (argc < 2) return 2;
    Devices::get().setDevChangedCallback(onChanged, nullptr);
    Devices::get().setEnableMdnsScan(false);
    std::shared_ptr<Device> sp;
    for (int i = 0; i < 30 && !sp; i++) {
        std::this_thread::sleep_for(std::chrono::milliseconds(500));
        for (auto &d : Devices::get().getDevList())
            if (d->productType() == product()) sp = d;
    }
    if (!sp) { mark("no camera of the selected product type found"); return 1; }
    Device *dev = sp.get();
    mark("found " + dev->devName());
    std::this_thread::sleep_for(std::chrono::seconds(1));
    std::string cmd = argv[1];
    auto tx = [&](int i) { return argc > i && atoi(argv[i]) == 2 ? Device::DevTX2 : Device::DevTX1; };
    char b[256];
    if (cmd == "info") {
        Device::DevTWSInfo t{}; mark("get TWS info");
        int r = twsInfo(dev, t);
        snprintf(b, sizeof b, "-> ret %d key %d func btn%d shock%d led%d sound%d | mute %d/%d ns %d/%d lvl %d/%d gain %d/%d batt %u/%u chg %u/%u",
                 r, t.key_cmd, t.func.button_enable, t.func.shock_enable, t.func.led_enable, t.func.sound_mode,
                 t.mic_info.mic1_mute, t.mic_info.mic2_mute, t.mic_info.mic1_ns, t.mic_info.mic2_ns,
                 t.mic1_ns_level, t.mic2_ns_level, t.mic1_gain, t.mic2_gain, t.mic1_batt_level, t.mic2_batt_level,
                 t.mic1_chg_status, t.mic2_chg_status);
        mark(b); pause();
        Device::AudioSelectAttr a{}; mark("get audio select");
        r = getAudioSelect(dev, a);
        snprintf(b, sizeof b, "-> ret %d support_auto %u is_auto %u has_pair_record %u", r, a.support_auto, a.is_auto, a.has_pair_record);
        mark(b); pause();
        unsigned char src = 0; mark("get selected audio source");
        r = getSelectedSource(dev, src);
        snprintf(b, sizeof b, "-> ret %d source %u", r, src); mark(b); pause();
        for (int i = 1; i <= 2; i++) {
            TX t2 = i == 1 ? Device::DevTX1 : Device::DevTX2;
            unsigned char lv = 0, ch = 0; bool mu = false; int g = 0; unsigned v = 0; std::string nm, sn;
            snprintf(b, sizeof b, "TX%d battery", i); mark(b); r = battLevel(dev, t2, lv);
            snprintf(b, sizeof b, "-> ret %d level %u", r, lv); mark(b); pause();
            snprintf(b, sizeof b, "TX%d charging", i); mark(b); r = charging(dev, t2, ch);
            snprintf(b, sizeof b, "-> ret %d status %u", r, ch); mark(b); pause();
            snprintf(b, sizeof b, "TX%d mute", i); mark(b); r = getMute(dev, t2, mu);
            snprintf(b, sizeof b, "-> ret %d mute %d", r, mu); mark(b); pause();
            snprintf(b, sizeof b, "TX%d gain", i); mark(b); r = getGain(dev, t2, g);
            snprintf(b, sizeof b, "-> ret %d gain %d", r, g); mark(b); pause();
            snprintf(b, sizeof b, "TX%d name", i); mark(b); r = getName(dev, t2, nm);
            snprintf(b, sizeof b, "-> ret %d name '%s'", r, nm.c_str()); mark(b); pause();
            snprintf(b, sizeof b, "TX%d version", i); mark(b); r = getVersion(dev, t2, v);
            snprintf(b, sizeof b, "-> ret %d version 0x%x", r, v); mark(b); pause();
            snprintf(b, sizeof b, "TX%d sn", i); mark(b); r = getSn(dev, t2, sn);
            snprintf(b, sizeof b, "-> ret %d sn '%s'", r, sn.c_str()); mark(b); pause();
        }
    } else if (cmd == "pair" && argc == 4) {
        snprintf(b, sizeof b, "set pair enabled TX%s %s", argv[2], argv[3]); mark(b);
        snprintf(b, sizeof b, "-> ret %d", setPair(dev, tx(2), atoi(argv[3]) != 0)); mark(b);
    } else if (cmd == "clearpair" && argc == 3) {
        snprintf(b, sizeof b, "clear paired info TX%s", argv[2]); mark(b);
        snprintf(b, sizeof b, "-> ret %d", clearPair(dev, tx(2))); mark(b);
    } else if (cmd == "mute" && argc == 4) {
        snprintf(b, sizeof b, "set mute TX%s %s", argv[2], argv[3]); mark(b);
        snprintf(b, sizeof b, "-> ret %d", setMute(dev, tx(2), atoi(argv[3]) != 0)); mark(b);
    } else if (cmd == "gain" && argc == 4) {
        snprintf(b, sizeof b, "set gain TX%s %s", argv[2], argv[3]); mark(b);
        snprintf(b, sizeof b, "-> ret %d", setGain(dev, tx(2), atoi(argv[3]))); mark(b);
    } else if (cmd == "key" && argc == 3) {
        snprintf(b, sizeof b, "set key type %s", argv[2]); mark(b);
        snprintf(b, sizeof b, "-> ret %d", setKey(dev, (Device::DevTWSKeyType)atoi(argv[2]))); mark(b);
    } else if (cmd == "source" && argc == 3) {
        snprintf(b, sizeof b, "set audio source %s", argv[2]); mark(b);
        snprintf(b, sizeof b, "-> ret %d", setSource(dev, atoi(argv[2]))); mark(b);
    } else if (cmd == "sound" && argc == 3) {
        snprintf(b, sizeof b, "set sound mode %s", argv[2]); mark(b);
        snprintf(b, sizeof b, "-> ret %d", setSound(dev, (Device::DevTWSSoundMode)atoi(argv[2]))); mark(b);
    } else if (cmd == "select" && argc == 3) {
        Device::AudioSelectParam p{}; p.is_auto = atoi(argv[2]);
        snprintf(b, sizeof b, "set audio select auto=%s", argv[2]); mark(b);
        snprintf(b, sizeof b, "-> ret %d", setAudioSelect(dev, p)); mark(b);
    } else if (cmd == "match") {
        mark("bluetooth match");
        snprintf(b, sizeof b, "-> ret %d", btMatch(dev)); mark(b);
    } else if (cmd == "rematch") {
        mark("bluetooth rematch");
        snprintf(b, sizeof b, "-> ret %d", btRematch(dev)); mark(b);
    } else if (cmd == "blepair" && argc == 4) {
        snprintf(b, sizeof b, "ble pairing enable %s %s", argv[2], argv[3]); mark(b);
        snprintf(b, sizeof b, "-> ret %d", blePairing(dev, atoi(argv[2]) != 0, atoi(argv[3]))); mark(b);
    } else if (cmd == "bleexit") {
        mark("ble pairing exit");
        snprintf(b, sizeof b, "-> ret %d", blePairingExit(dev)); mark(b);
    } else {
        mark("bad arguments"); return 2;
    }
    std::this_thread::sleep_for(std::chrono::milliseconds(1500));
    mark("done");
    return 0;
}
