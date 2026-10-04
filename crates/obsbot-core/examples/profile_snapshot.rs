// Temporary: prints every built-in profile after inheritance, to prove a
// refactor leaves them unchanged. Not committed.
fn main() {
    let mut profiles = obsbot_core::DeviceProfile::builtin();
    profiles.sort_by(|a, b| a.id.cmp(&b.id));
    for p in profiles {
        println!("=== {} ({})", p.id, p.name);
        println!("matches {:?}", p.matches);
        println!(
            "system_info {} event_queue {} wireless_mics {}",
            p.system_info, p.event_queue, p.wireless_mics
        );
        println!("presets {:?}", p.presets);
        println!("gimbal_velocity {:?}", p.gimbal_velocity);
        println!("firmware {:?}", p.firmware);
        for (id, b) in &p.features {
            println!("  {:?} => {:?}", id, b);
        }
    }
}
