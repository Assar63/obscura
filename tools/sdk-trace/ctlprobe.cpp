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
