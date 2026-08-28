// Subsystem Windows : masque la console en release, garde-la visible en debug.
// Cela évite l'apparition d'une fenêtre CMD parasite au démarrage de l'exe.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    dictaku_lib::run()
}
