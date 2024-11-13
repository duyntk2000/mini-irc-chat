use anyhow::Result;
use crypto_box::PublicKey;
use mini_irc_protocol::{
    AsyncTypedReader, AsyncTypedWriter, BroadcastReceiverWithList, BroadcastSenderWithList, ChanOp,
    MessageReceiver, Request, Response,
};
use serde_encrypt::{
    key::key_pair::ReceiverKeyPair, shared_key::SharedKey, traits::SerdeEncryptPublicKey,
    EncryptedMessage, ReceiverCombinedKey, ReceiverKeyPairCore,
};
use serde_encrypt_core::key::key_pair::public_key::SenderPublicKey;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

type DB = Arc<Mutex<HashSet<String>>>;
type DBChan = Arc<Mutex<HashMap<String, BroadcastSenderWithList<Response, String>>>>;
#[tokio::main]
async fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6379").await?;

    let db: DB = Arc::new(Mutex::new(HashSet::new()));
    let db_chan: DBChan = Arc::new(Mutex::new(HashMap::new()));
    loop {
        let (socket, _) = listener.accept().await?;
        let db = db.clone();
        let db_chan = db_chan.clone();
        tokio::spawn(async move {
            process(socket, db, db_chan).await;
        });
    }
}

fn error(message: String) -> Response {
    Response::Error(message)
}

async fn connect_user(username: String, db: DB) -> Option<Response> {
    let mut db = db.lock().unwrap();
    if db.insert(username) {
        Some(Response::AckConnect("Welcome".to_string()))
    } else {
        None
    }
}

async fn disconnect_user(username: String, db: DB) {
    if !username.is_empty() {
        let mut db = db.lock().unwrap();
        db.remove(&username);
    }
}

async fn add_user_to_chan(
    username: &str,
    channel: String,
    db_chan: DBChan,
) -> Option<BroadcastReceiverWithList<Response, String>> {
    let mut db_chan = db_chan.lock().unwrap();
    if db_chan.contains_key(&channel) {
        let users = db_chan.get_mut(&channel).unwrap();
        users.subscribe(username.to_string())
    } else {
        let mut users = BroadcastSenderWithList::<Response, String>::new(32);
        let reciever = users.subscribe(username.to_string());
        db_chan.insert(channel, users);
        reciever
    }
}

async fn remove_user_from_chan(username: &str, channel: String, db_chan: DBChan) {
    let res = Response::Channel {
        op: ChanOp::UserDel(username.to_string()),
        chan: channel.clone(),
    };
    let mut db_chan = db_chan.lock().unwrap();
    let _ = db_chan.get_mut(&channel).unwrap().send(res);
}

async fn message_to_chan(username: &str, channel: String, content: String) -> Response {
    Response::Channel {
        op: ChanOp::Message {
            from: username.to_string(),
            content,
        },
        chan: channel,
    }
}

async fn process(socket: TcpStream, db: DB, db_chan: DBChan) {
    let key_pair = ReceiverKeyPair::generate();
    let mut combined: Option<ReceiverCombinedKey> = None;
    let mut shared: SharedKey;
    let mut public_key_other: SenderPublicKey;
    let (reader, writer) = socket.into_split();
    let mut typed_reader = AsyncTypedReader::<_, Request>::new(reader);
    let mut typed_writer = AsyncTypedWriter::<_, Response>::new(writer);
    let mut user: String = "".to_string();
    let mut channels: Vec<String> = Vec::new();

    // Channel pour gérer communication avec Broadcast
    let (tx, mut rx) = mpsc::channel(32);

    loop {
        let res: Option<Response> = tokio::select! {
            val = typed_reader.recv() => {
                if val.is_err() {
                    drop(rx);
                    drop(tx);
                    break;
                }
                let rq = val.unwrap().unwrap();
                let db = db.clone();
                let db_chan = db_chan.clone();
                let response = match rq {
                    Request::Secure(key) => {
                        let key_bytes: [u8; 32] = key.try_into().unwrap();
                        public_key_other = SenderPublicKey::from(PublicKey::from(key_bytes));
                        combined = Some(ReceiverCombinedKey::new(&public_key_other, key_pair.private_key()));
                        Response::Secure(key_pair.public_key().as_ref().as_bytes().to_vec())
                    },
                    Request::Shared(key) => {
                        if combined.is_some() {
                            let encrypted_message = EncryptedMessage::deserialize(key).unwrap();
                            shared = SharedKey::decrypt_owned(&encrypted_message, &combined.clone().unwrap()).unwrap();
                            typed_reader.set_shared_key(shared.clone());
                            typed_writer.set_shared_key(shared.clone());
                            Response::Ack
                        } else {
                            error("invalid".to_string())
                        }
                    }
                    Request::Connect(username) => {
                        if let Some(res) = connect_user(username.clone(), db).await {
                            user = username.clone();
                            res
                        } else {
                            error("Invalid username".to_string())
                        }
                    },
                    Request::JoinChan(channel) => {
                        if user.is_empty() {
                            error("Please connect first".to_string())
                        } else if let Some(mut reciever) = add_user_to_chan(&user, channel.clone(), db_chan.clone()).await {
                            let users = reciever.into_subscribers().clone();
                            let tx2 = tx.clone();
                            let _ = db_chan
                                        .lock()
                                        .unwrap()
                                        .get_mut(&channel)
                                        .unwrap()
                                        .send(Response::Channel { op: ChanOp::UserAdd(user.clone()), chan: channel.clone() });
                            let user = user.clone();

                            // Spawn un thread pour transferer messages de Broadcast
                            tokio::spawn(async move {
                                loop {
                                    let mess = reciever.recv().await;
                                    match mess {
                                        Ok(m) => {
                                            if let Response::Channel {op: ChanOp::UserDel(target), chan: _} = m.clone() {
                                                if target == user {
                                                    break;
                                                }
                                            }
                                            let _ = tx2.send(m).await;
                                        },
                                        Err(_) => break,
                                    }
                                }
                                drop(tx2);
                                drop(reciever);
                            });
                            channels.push(channel.clone());
                            Response::AckJoin { chan: channel, users }
                        } else {
                            error("User already in channel".to_string())
                        }
                    },
                    Request::LeaveChan(channel) => {
                        if user.is_empty() {
                            error("Please connect first".to_string())
                        } else {
                            remove_user_from_chan(&user, channel.clone(), db_chan.clone()).await;
                            Response::AckLeave(user.clone())
                        }
                    },
                    Request::Message { to: MessageReceiver::Channel(channel), content } => {
                        if user.is_empty() {
                            error("Please connect first".to_string())
                        } else {
                            let mess = message_to_chan(&user, channel.clone(), content).await;
                            let _ = db_chan.lock().unwrap().get_mut(&channel).unwrap().send(mess.clone());
                            mess
                        }
                    },
                    Request::Message { to: MessageReceiver::User(_user), content: _content } => {
                        todo!();
                    },
                };
                Some(response)
            },
            Some(mess) = rx.recv() => {
                if let Response::Channel{op: ChanOp::Message{from: target, content: _},chan: _} = mess.clone() {
                    if target != user {
                        Some(mess)
                    } else {
                        None
                    }
                } else if let Response::Channel{op: _, chan: _} = mess.clone() {
                    Some(mess)
                } else {
                    None
                }
            }
            else => break,
        };
        if let Some(r) = res {
            let e = typed_writer.send(&r).await;
            if e.is_err() {
                break;
            }
        }
    }
    println!("user {} disconnect", user);
    let db = db.clone();
    let db_chan = db_chan.clone();
    disconnect_user(user.clone(), db).await;
    for chan in channels.into_iter() {
        let db_chan = db_chan.clone();
        remove_user_from_chan(&user, chan, db_chan).await;
    }
}
