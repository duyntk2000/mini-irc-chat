use mini_irc_ui::{App, KeyReaction};
use std::error::Error;
fn main() -> Result<(), Box<dyn Error>> {
    // Etape 1: créer la structure
    let mut app = App::default();

    // Optionnel: on ajoute des onglets et des utilisateur par "room"
    // afin de montrer l'API de App
    app.add_tab("#general".into());
    app.add_tab("tab 2".into());
    app.add_tab("tab 3".into());

    for tab in &["#general", "tab 2", "tab 3"] {
        app.add_user("Foo".into(), tab.to_string());
        app.add_user("BarFoo".into(), tab.to_string());
        app.add_user("Baz".into(), tab.to_string());
    }

    // Etape 2: on démarre la TUI
    app.start()?;

    loop {
        // Etape 3: on dessine l'application (à faire après chaque évènement lu,
        // y compris des changements de taille de la fenêtre !)
        app.draw()?;

        // Etape 4: on modifie l'état interne de l'application, en fonction des évènements
        // clavier / système. Ici, l'interface est très simple: suite à un évènement, soit:
        // - l'évènement est géré en interne de App, il n'y a rien à faire
        // - soit l'utilisateur veut quitter l'application, il faut interrompre la boucle et retourner
        // - soit l'utilisateur souhaite envoyer un message depuis l'interface vers le bon "room"
        if let Ok(e) = crossterm::event::read() {
            match app.react_to_event(e) {
                Some(KeyReaction::Quit) => {
                    break;
                }
                Some(KeyReaction::UserInput(s)) => {
                    // TODO pour l'instant, le message à envoyer est simplement affiché localement
                    // Il faudra l'envoyer au serveur IRC
                    // TODO (plus tard) comment traiter les demandes pour rejoindre / quitter une room ?
                    let current_tab = app.get_current_tab();
                    app.push_message("test".to_string(), s, current_tab);
                }
                None => {} // Rien à faire, géré en interne
            }
        }
    }
    Ok(())
}
