use mini_irc_protocol::{MessageReceiver, Request};
use mini_irc_ui::App;

pub fn handle_user_input(input: String, app: &mut App) -> Result<Option<Request>, String> {
    if input.starts_with('/') {
        // On a reçu une commande.
        if input.starts_with("/join") {
            match input.strip_prefix("/join ") {
                Some(chan) => Ok(Some(Request::JoinChan(chan.to_string()))),
                None => Err(
                    "The command 'join' has to be used with the name of a channel to join."
                        .to_string(),
                ),
            }
        } else if input.starts_with("/quit") {
            let s = app.get_current_tab();
            if s.is_empty() {
                Err("Can't quit. No channel joined.".to_string())
            } else {
                match s.parse() {
                    Ok(MessageReceiver::Channel(chan)) => Ok(Some(Request::LeaveChan(chan))),
                    Ok(MessageReceiver::User(_)) => {
                        todo!("What does it mean to leave DM from one user?")
                    }
                    Err(e) => Err(e),
                }
            }
        } else if input.starts_with("/clear notif") {
            app.clear_notif();
            Ok(None)
        } else if input.starts_with("/to") {
            let res = input.splitn(3, ' ').collect::<Vec<_>>();
            let username = res[1].to_string();
            let msg = res[2].to_string();
            let tab_name = format!("@{username}");
            app.add_tab(tab_name.clone());
            app.push_message("myself".into(), msg.clone(), tab_name);
            Ok(Some(Request::Message {
                to: MessageReceiver::User(username),
                content: msg,
            }))
        } else {
            Err(format!("Not a command: {input}"))
        }
    } else {
        // On a reçu un message pour le tab courant.
        // Pour le moment, on ne gère que le cas des channels.

        Ok(Some(Request::Message {
            to: app.get_current_tab().parse()?,
            content: input,
        }))
    }
}
