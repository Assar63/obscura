// Drive the OBSBOT SDK's gesture API (aiGetGestureParaR / aiSetGestureParaR)
// for traffic recording with xulog.so. Types are DevGestureParaType:
// 0 master, 1 target selection, 2 zoom, 3 dynamic zoom, 4 record,
// 5 snapshot, 6 rolling, 7 mirror, 8 zoom factor (float).
//
// gprobe get                    read types 0-3, 7 and 8
// gprobe set <type> <0|1|float>
//
// Product type defaults to the Tiny 3; set OBSBOT_PRODUCT=<ObsbotProductType
// number> to drive another model.
#include <dev/devs.hpp>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <thread>

static const char *kNames[] = {"Gesture", "TargetSelection", "Zoom", "DynamicZoom", "Record",
                               "Snapshot", "Rolling", "Mirror", "ZoomFactor"};
static void mark(const char *s) { fprintf(stderr, "=== %s\n", s); fflush(stderr); }
static void onChanged(std::string, bool, void *) {}
static ObsbotProductType product() {
    const char *p = getenv("OBSBOT_PRODUCT");
    return p ? (ObsbotProductType)atoi(p) : ObsbotProdTiny3;
}

int main(int argc, char **argv) {
    if (argc < 2) return 2;
    Devices::get().setDevChangedCallback(onChanged, nullptr);
    Devices::get().setEnableMdnsScan(false);
    // Discovery takes a few seconds; the product type is known only after.
    std::shared_ptr<Device> dev;
    for (int i = 0; i < 30 && !dev; i++) {
        std::this_thread::sleep_for(std::chrono::milliseconds(500));
        for (auto &d : Devices::get().getDevList())
            if (d->productType() == product()) dev = d;
    }
    if (!dev) { mark("no camera of the selected product type found"); return 1; }
    mark(("found " + dev->devName() + " sn " + dev->devSn()).c_str());
    std::this_thread::sleep_for(std::chrono::seconds(1));
    char buf[128];
    if (!strcmp(argv[1], "get")) {
        int types[] = {0, 1, 2, 3, 7};
        for (int t : types) {
            snprintf(buf, sizeof buf, "get %s (bool)", kNames[t]); mark(buf);
            bool b = false;
            int r = dev->aiGetGestureParaR((Device::DevGestureParaType)t, b);
            snprintf(buf, sizeof buf, "-> ret %d value %d", r, b); mark(buf);
            std::this_thread::sleep_for(std::chrono::milliseconds(500));
        }
        mark("get ZoomFactor (float)");
        float f = 0;
        int r = dev->aiGetGestureParaR(Device::DevGestureParaTypeZoomFactor, f);
        snprintf(buf, sizeof buf, "-> ret %d value %.2f", r, f); mark(buf);
    } else if (!strcmp(argv[1], "set") && argc == 4) {
        int t = atoi(argv[2]), r;
        if (t == 8) {
            float f = atof(argv[3]);
            snprintf(buf, sizeof buf, "set %s %.2f", kNames[t], f); mark(buf);
            r = dev->aiSetGestureParaR(Device::DevGestureParaTypeZoomFactor, f);
        } else {
            bool b = atoi(argv[3]) != 0;
            snprintf(buf, sizeof buf, "set %s %d", kNames[t], b); mark(buf);
            r = dev->aiSetGestureParaR((Device::DevGestureParaType)t, b);
        }
        snprintf(buf, sizeof buf, "-> ret %d", r); mark(buf);
    } else {
        mark("bad arguments");
        return 2;
    }
    // The SDK sends some writes from a worker thread; give it time.
    std::this_thread::sleep_for(std::chrono::milliseconds(1500));
    mark("done");
    return 0;
}
