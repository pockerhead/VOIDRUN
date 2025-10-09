//! Headless симуляция VOIDRUN
//!
//! Запускает Bevy App без рендера для тестирования детерминизма

use voidrun_simulation::create_headless_app;

fn main() {
    let seed = 42;
    println!("Starting VOIDRUN headless simulation (seed: {})", seed);

    let mut app = create_headless_app(seed);

    // Запускаем 1000 тиков симуляции
    for tick in 0..1000 {
        app.update();

        if tick % 100 == 0 {
            let entity_count = app.world().entities().len();
            println!("Tick {}: {} entities", tick, entity_count);
        }
    }

    println!("Simulation complete!");
}
