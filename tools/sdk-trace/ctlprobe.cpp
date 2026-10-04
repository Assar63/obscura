// Drive the OBSBOT SDK's camera and AI settings that are declared in its
// header, for traffic recording with xulog.so: field of view, AI mode and
// sub-mode, tracking mode/speed, control parameters, voice control, run
// status.
//
// ctlprobe fov <0|1|2>                 cameraSetFovU (86/78/65 degrees)
// ctlprobe aimode <mode> <sub>         cameraSetAiModeU (AiWorkModeType)
// ctlprobe trackmode <n>               aiSetTrackingModeR (AiVerticalTrackType)
// ctlprobe trackspeed <n>              aiSetTrackSpeedTypeR (AiTrackSpeedType)
// ctlprobe ctlget <target> <para>      aiGetControlParaR (bool, int, float)
// ctlprobe ctlbool|ctlint|ctlfloat <target> <para> <value>
// ctlprobe voice <cmd> <state>         cameraSetAudioCtrlStateU
// ctlprobe runstatus <n>               cameraSetDevRunStatusR (DevStatus)
// ctlprobe audiomode <mode>            cameraSetAudioModeU (AudioModeType)
// ctlprobe doafindback <0|1>           cameraSetDoaFindBack
// ctlprobe doarange <n>                cameraSetDoaRange
// ctlprobe audiodistance <n>           cameraSetAudioDistanceU
// ctlprobe wbget                       cameraGetWhiteBalanceR (both overloads)
// ctlprobe wbset <type> <param>        cameraSetWhiteBalanceR (DevWhiteBalanceType)
// ctlprobe twsfunc <type> <0|1> <param>  cameraSetTWSFuncR (DevTWSFuncType)
// ctlprobe twssound <mode>             cameraSetTWSSoundModeR (DevTWSSoundMode)
// ctlprobe gimget <type>               aiGetGimbalParaR (bool and float)
// ctlprobe gimbool|gimfloat <type> <value>  aiSetGimbalParaR
// ctlprobe bootmode <mode> <sub>       cameraSetBootModeU (AiWorkModeType, AiSubModeType)
// ctlprobe yawrev <0|1>                aiSetGimbalYawDirReverseR
// ctlprobe gimbalstate                 aiGetGimbalStateR, gimbalGetAttitudeInfoR
// ctlprobe presetlist                  aiGetGimbalPresetListR
// ctlprobe presetinfo <id>             aiGetGimbalPresetInfoWithIdR
// ctlprobe presettrg <id>              aiTrgGimbalPresetR
// ctlprobe presetadd <id> <name> [yaw pitch zoom]  aiAddGimbalPresetR
// ctlprobe presetname <id> <name>      aiSetGimbalPresetNameWithIdR
// ctlprobe presetdel <id>              aiDelGimbalPresetR
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

// Exported by libdev but not declared in its header: non-virtual members,
// called with the device as the first argument (Itanium C++ ABI).
extern "C" int32_t _ZN6Device19cameraSetAudioModeUENS_9AudioModeE(Device *, Device::AudioMode);
extern "C" int32_t _ZN6Device20cameraSetDoaFindBackEh(Device *, unsigned char);
extern "C" int32_t _ZN6Device17cameraSetDoaRangeEh(Device *, unsigned char);
extern "C" int32_t _ZN6Device23cameraSetAudioDistanceUEh(Device *, unsigned char);
extern "C" int32_t _ZN6Device17cameraSetTWSFuncRENS_14DevTWSFuncTypeEbs(Device *, Device::DevTWSFuncType, bool,
                                                                        short);
extern "C" int32_t _ZN6Device22cameraSetTWSSoundModeRENS_15DevTWSSoundModeE(Device *, Device::DevTWSSoundMode);

static void mark(const std::string &s) { fprintf(stderr, "=== %s\n", s.c_str()); fflush(stderr); }
static void onChanged(std::string, bool, void *) {}
static ObsbotProductType product() {
    const char *p = getenv("OBSBOT_PRODUCT");
    return p ? (ObsbotProductType)atoi(p) : ObsbotProdTiny3;
}

int main(int argc, char **argv) {
    if (argc < 2) return 2;
    Devices::get().setDevChangedCallback(onChanged, nullptr);
    Devices::get().setEnableMdnsScan(false);
    std::shared_ptr<Device> dev;
    for (int i = 0; i < 30 && !dev; i++) {
        std::this_thread::sleep_for(std::chrono::milliseconds(500));
        for (auto &d : Devices::get().getDevList())
            if (d->productType() == product()) dev = d;
    }
    if (!dev) { mark("no camera of the selected product type found"); return 1; }
    mark("found " + dev->devName());
    std::this_thread::sleep_for(std::chrono::seconds(1));

    std::string cmd = argv[1];
    auto arg = [&](int i) { return argc > i ? atoi(argv[i]) : 0; };
    auto target = [&] { return (Device::DevControlTargetType)arg(2); };
    auto para = [&] { return (Device::DevControlParaType)arg(3); };
    std::string what = cmd;
    for (int i = 2; i < argc; i++) what += std::string(" ") + argv[i];
    mark(what);
    int r = -100;
    if (cmd == "fov" && argc == 3) {
        r = dev->cameraSetFovU((Device::FovType)arg(2));
    } else if (cmd == "aimode" && argc == 4) {
        r = dev->cameraSetAiModeU((Device::AiWorkModeType)arg(2), arg(3));
    } else if (cmd == "trackmode" && argc == 3) {
        r = dev->aiSetTrackingModeR((Device::AiVerticalTrackType)arg(2));
    } else if (cmd == "trackspeed" && argc == 3) {
        r = dev->aiSetTrackSpeedTypeR((Device::AiTrackSpeedType)arg(2));
    } else if (cmd == "ctlget" && argc == 4) {
        bool b = false; int n = 0; float f = 0;
        int rb = dev->aiGetControlParaR(target(), para(), b);
        int rn = dev->aiGetControlParaR(target(), para(), n);
        int rf = dev->aiGetControlParaR(target(), para(), f);
        char buf[160];
        snprintf(buf, sizeof buf, "-> bool ret %d %d | int ret %d %d | float ret %d %g", rb, b, rn, n, rf, f);
        mark(buf);
    } else if (cmd == "ctlbool" && argc == 5) {
        r = dev->aiSetControlParaR(target(), para(), arg(4) != 0);
    } else if (cmd == "ctlint" && argc == 5) {
        r = dev->aiSetControlParaR(target(), para(), arg(4));
    } else if (cmd == "ctlfloat" && argc == 5) {
        r = dev->aiSetControlParaR(target(), para(), (float)atof(argv[4]));
    } else if (cmd == "voice" && argc == 4) {
        r = dev->cameraSetAudioCtrlStateU((Device::AudioCtrlCmdType)arg(2), arg(3));
    } else if (cmd == "runstatus" && argc == 3) {
        r = dev->cameraSetDevRunStatusR((Device::DevStatus)arg(2));
    } else if (cmd == "audiomode" && argc == 3) {
        Device::AudioMode m{};
        m.mode = (uint8_t)arg(2);
        r = _ZN6Device19cameraSetAudioModeUENS_9AudioModeE(dev.get(), m);
    } else if (cmd == "doafindback" && argc == 3) {
        r = _ZN6Device20cameraSetDoaFindBackEh(dev.get(), (unsigned char)arg(2));
    } else if (cmd == "doarange" && argc == 3) {
        r = _ZN6Device17cameraSetDoaRangeEh(dev.get(), (unsigned char)arg(2));
    } else if (cmd == "audiodistance" && argc == 3) {
        r = _ZN6Device23cameraSetAudioDistanceUEh(dev.get(), (unsigned char)arg(2));
    } else if (cmd == "wbget" && argc == 2) {
        Device::DevWhiteBalanceType t{};
        int32_t p = 0;
        int r1 = dev->cameraGetWhiteBalanceR(t, p);
        char buf[200];
        snprintf(buf, sizeof buf, "-> simple ret %d: type %d param %d", r1, (int)t, p);
        mark(buf);
        Device::WhiteBalanceSetting w{};
        int r2 = dev->cameraGetWhiteBalanceR(w);
        snprintf(buf, sizeof buf, "-> full ret %d: mode %d param %d manual_gain %d b %d r %d xab %d ygm %d", r2,
                 (int)w.light_mode, w.param, w.is_manual_gain, w.user_manual_b_gain, w.user_manual_r_gain,
                 w.xab_offset, w.ygm_offset);
        mark(buf);
    } else if (cmd == "wbset" && argc == 4) {
        r = dev->cameraSetWhiteBalanceR((Device::DevWhiteBalanceType)arg(2), arg(3));
    } else if (cmd == "twsfunc" && argc == 5) {
        r = _ZN6Device17cameraSetTWSFuncRENS_14DevTWSFuncTypeEbs(dev.get(), (Device::DevTWSFuncType)arg(2),
                                                                arg(3) != 0, (short)arg(4));
    } else if (cmd == "twssound" && argc == 3) {
        r = _ZN6Device22cameraSetTWSSoundModeRENS_15DevTWSSoundModeE(dev.get(), (Device::DevTWSSoundMode)arg(2));
    } else if (cmd == "gimget" && argc == 3) {
        bool b = false;
        float f = 0;
        auto t = (Device::DevGimbalParaType)arg(2);
        int rb = dev->aiGetGimbalParaR(t, b);
        int rf = dev->aiGetGimbalParaR(t, f);
        char buf[160];
        snprintf(buf, sizeof buf, "-> bool ret %d %d | float ret %d %g", rb, b, rf, f);
        mark(buf);
    } else if (cmd == "gimbool" && argc == 4) {
        r = dev->aiSetGimbalParaR((Device::DevGimbalParaType)arg(2), arg(3) != 0);
    } else if (cmd == "gimfloat" && argc == 4) {
        r = dev->aiSetGimbalParaR((Device::DevGimbalParaType)arg(2), (float)atof(argv[3]));
    } else if (cmd == "bootmode" && argc == 4) {
        r = dev->cameraSetBootModeU((Device::AiWorkModeType)arg(2), (Device::AiSubModeType)arg(3));
    } else if (cmd == "yawrev" && argc == 3) {
        r = dev->aiSetGimbalYawDirReverseR(arg(2) != 0);
    } else if (cmd == "gimbalstate" && argc == 2) {
        Device::AiGimbalStateInfo g{};
        r = dev->aiGetGimbalStateR(&g);
        char buf[200];
        snprintf(buf, sizeof buf, "-> euler r %g p %g y %g | motor r %g p %g y %g", g.roll_euler,
                 g.pitch_euler, g.yaw_euler, g.roll_motor, g.pitch_motor, g.yaw_motor);
        mark(buf);
        float xyz[3] = {0, 0, 0};
        int ra = dev->gimbalGetAttitudeInfoR(xyz);
        snprintf(buf, sizeof buf, "-> attitude ret %d: %g %g %g", ra, xyz[0], xyz[1], xyz[2]);
        mark(buf);
    } else if (cmd == "presetlist" && argc == 2) {
        Device::DevDataArray ids{};
        r = dev->aiGetGimbalPresetListR(&ids);
        std::string l = "-> ids:";
        for (int i = 0; i < ids.len && i < 16; i++) l += " " + std::to_string(ids.data_int32[i]);
        mark(l + " (len " + std::to_string(ids.len) + ")");
    } else if (cmd == "presetinfo" && argc == 3) {
        Device::PresetPosInfo p{};
        r = dev->aiGetGimbalPresetInfoWithIdR(&p, arg(2));
        char buf[200];
        snprintf(buf, sizeof buf, "-> id %d roll %g pitch %g yaw %g zoom %g name '%.*s'", p.id, p.roll,
                 p.pitch, p.yaw, p.zoom, p.name_len > 0 && p.name_len <= 64 ? p.name_len : 0, p.name);
        mark(buf);
    } else if (cmd == "presettrg" && argc == 3) {
        r = dev->aiTrgGimbalPresetR(arg(2));
    } else if (cmd == "presetadd" && (argc == 4 || argc == 7)) {
        Device::PresetPosInfo p{};
        p.id = arg(2);
        if (argc == 7) {
            p.yaw = (float)atof(argv[4]);
            p.pitch = (float)atof(argv[5]);
            p.zoom = (float)atof(argv[6]);
        }
        p.name_len = (int)strnlen(argv[3], 63);
        memcpy(p.name, argv[3], p.name_len);
        r = dev->aiAddGimbalPresetR(&p);
    } else if (cmd == "presetname" && argc == 4) {
        r = dev->aiSetGimbalPresetNameWithIdR(argv[3], arg(2));
    } else if (cmd == "presetdel" && argc == 3) {
        r = dev->aiDelGimbalPresetR(arg(2));
    } else {
        mark("bad arguments");
        return 2;
    }
    if (r != -100) mark("-> ret " + std::to_string(r));
    // The SDK sends some writes from a worker thread; give it time.
    std::this_thread::sleep_for(std::chrono::milliseconds(1500));
    mark("done");
    return 0;
}
