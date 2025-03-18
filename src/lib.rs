use htmlescape::decode_html;
use memchr::memmem;
use percent_encoding::percent_decode_str;
use std::{
    fs,
    sync::Mutex,
    path::{Path, PathBuf},
    io::{prelude::*, Read, Write},
    net::{TcpListener, TcpStream},
};

use lazy_static::lazy_static;
use zip::write::SimpleFileOptions;
use walkdir::WalkDir;

use encoding::all::WINDOWS_1252;
use encoding::{DecoderTrap, Encoding};

use rusqlite::{params, Result, Connection};

use std::thread;
// use std::time::Duration;
// use std::path::Path;

// start working on handling conection

/*
--------------------------------------------------------------------------------
    every successfull response will go to response()
    every time smth fails will go to error()

    >>>>>>>>implement auth checking like ASAP

--------------------------------------------------------------------------------
*/

pub fn handle_connection(mut stream: TcpStream){
    let mut buffer = vec![0u8; 2048];  //pre defined
    let mut received_data = Vec::new(); //growable

    

    loop {

        let bytes_read = match stream.read(&mut buffer) {
            Ok(b) => b,
            Err(e) => {
                println!("Failed to read request");
                break;
            },
        };


        if bytes_read == 0 {
            break;
        }

        received_data.extend_from_slice(&buffer[..bytes_read]);

        if received_data[..3] == *b"GET" && bytes_read < buffer.len() {
            get(stream, received_data);
            break;
        }

        if received_data[..4] == *b"POST" && bytes_read < buffer.len() {
            post(stream, received_data);
            break;
        }

    }
}

fn get(mut stream: TcpStream, buffer: Vec<u8>){
    println!("Get resposne identified");
    let status_code = "HTTP/1.1 200 OK\r\n";

    let connected = if let Some(_) = memmem::find(&buffer[..], b"Cookie: Auth").map(|p| p as usize) {
        true
    }   else {
        false
    };

    println!("\n\nrequest is = {}", String::from_utf8_lossy(&buffer[..]));

    if connected == false {
        let response = format!("{}\r\n{}", status_code, login());
        // println!("response = \n{}", response);

        respond(stream, response.to_string());
    } else if &buffer[..6] == b"GET / "{
        let response = format!("{}\r\n{}", status_code, list());

        respond(stream, response.to_string());
    } else if &buffer[..13] == b"GET /messages" {
        println!("Here i should fetch messages");
    }else {
        println!("\n\n\n\nchat whatever \n\n\n");
        let c = memmem::find(&buffer[..], b"GET /").map(|p| p as usize).unwrap();
        let c = &buffer[c + "GET /".len()..]; 
        let end = memmem::find(&c[..], b" ").map(|p| p as usize).unwrap();
        println!("end = {}", end);
        let c = &c[..end];
        let response = format!("{}{}", status_code, chat(String::from_utf8_lossy(&c[..]).to_string()));
        respond(stream, response.to_string());
    }

    
}

fn post(mut stream: TcpStream, buffer: Vec<u8>){
    println!("POST resposne identified");
    let status_code = "HTTP/1.1 200 OK\r\n\r\n";

    // let conn = Connection::open("chats/chat.db").unwrap();

    println!("\n\nrequest is = {}", String::from_utf8_lossy(&buffer[..]));

    if let Some(new_chat) = memmem::find(&buffer[..], b"new_chat=").map(|p| p as usize) {
        let new_chat = String::from_utf8_lossy(&buffer[new_chat + "new_chat=".len() ..] );
        println!("New chat found ={}", new_chat);
        setup_chat(new_chat.replace("+", " ").trim().to_string());

        let response = format!("{}{}", status_code, list());
        respond(stream, response.to_string());

    } else if let Some(_) = memmem::find(&buffer[..], b"username="){
        let response = connect(buffer);
        respond(stream, response.to_string());
    }
    
    
}


// -------------------------------------------------------------------

fn connect(buffer: Vec<u8>) -> String {
    let buffer = &buffer[..];

    let data = memmem::find(buffer, b"username=").map(|p| p as usize).unwrap();
    let data = &buffer[data..];

    let mut stops = memmem::find_iter(data, b"&").map(|p| p as usize);
    let username = &data["username=".len()..stops.next().unwrap()];

    let stop = stops.next().unwrap();

    let password = memmem::find(data, b"password=").map(|p| p as usize).unwrap();
    let password = &data[password + "password=".len() .. stop]; 

    let color = &data[stop + "&color=%23".len() ..];
    let color = String::from(format!("#{}", String::from_utf8_lossy(&color[..])));

    let username = String::from_utf8_lossy(&username[..]);

    println!("username={}", username);
    println!("password={}", String::from_utf8_lossy(&password[..]));
    println!("color={}", color);
    println!("\n\n\n");

    let status = "HTTP/1.1 200 Ok\r\n";
    let cookie1 = format!("Set-Cookie: Auth=\"user-{}-token\"; Path=/; HttpOnly; SameSite=Strict;\r\n", username);
    let cookie2 = format!("Set-Cookie: Color=\"color-{}-token\"; Path=/; HttpOnly; SameSite=Strict;\r\n", color);

    let response = format!("{}{}{}Location: /\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n\r\n{}", status, cookie1, cookie2, list());

    response.to_string()
}

fn respond(mut stream: TcpStream, response: String) {
    println!("response =\n{}", response);

    stream.write(response.as_bytes()).unwrap();
    stream.flush().unwrap(); 
}
/*
    ---------------------------------------------------------------------------------------
    chat, chats list, login and register

    errors
*/


fn login() -> String {
    let mut html = String::from("<!DOCTYPE html>
    <html>
    <head>
        <title> Ligon/ Sign up </title>
    </head>
    <body>
        <h1> Welcome to yesterday's era of comunication </h1>
        <h3> login or sign up to continue </h3>
        <form action=\"/\" method=\"POST\">
            <input placeholder=\"Username:\" type=\"text\" name=\"username\">
            <input placeholder=\"Password:\" type=\"password\" name =\"password\">
            <input type=\"color\" name=\"color\">
            <button type=\"submit\"> Login/Sign up </button>
        </form>
    
    </body>
    </html>
    ");
    html
}

// this is actually for the chat
fn chat(chat: String) -> String {

    println!("chat = {:?}", chat);
    let mut html = String::from("<!DOCTYPE html>
    <html>
    <head>
        <title> Chat </title>
    <head>
    <body>
        <h2>Welcome to the chat fam, enjoy :)</h2>
        <ul> 
    ");
    println!("chat = chats/{}.db", chat);
    let conn = Connection::open(format!("chats/{}.db", chat)).unwrap();

    // Insert a new message
    // insert_message(&conn, "adam", "#fe02aa", "HOW YOU DOINGGG").unwrap();
    // insert_message(&conn, "NIGA", "#00ff00", "Sup NIGGASSS!").unwrap();


    // Retrieve and print all messages
    let messages = get_messages(&conn).unwrap();
    for message in messages {
        println!("{:?}", message);
        let name = message.name;
        let color = message.color;
        let message = message.message;
        html.push_str(&*format!("
            <li>
                <p style=\"color:{};\"> {} </p>

                <h4> {} </h4>
            </li>
        ",color,name, message))

    }

    html.push_str("
        <script>
        async function fetchMessage(){
            await fetch('/messages') ;
        }
            setInterval( fetchMessage , 1000);
        </script>

        </body>
        </html>
    ");

    let cookie = format!("Set-Cookie: Chat_room=\"chats/{}.db\"; Path=/ ; HttpOnly; SameSite=Strict\r\n", chat);

    let html = String::from(format!("{}\r\n\r\n{}", cookie, html));
    
    html
}

fn list() -> String {
    let mut html = String::from("<!DOCTYPE html>
    <html>
    <head>
        <title> Chat lists </title>
    <head>
    <body>
        <h2> The available chat rooms </h2>
        <ul>
    ");

    let available_chats = fs::read_dir("chats/").unwrap();

    let mut chats = Vec::new();
    for i in available_chats {
        chats.push(i.unwrap().file_name().into_string().unwrap());
    }

    if chats.len() == 0 {
        html.push_str("
        <h2> Oh no, it seems like no chat rooms are available</h2
        <h3> Create a new chat and start chatting :)</h3>
        ");
    }
    for chat in chats {
        println!("chats = {:?}", chat);
        let chat = chat.clone().into_bytes();
        let chat = String::from_utf8_lossy(&chat[..chat.len() - 3]);

        html.push_str(&*format!("
        <li>
            {}
            <button  onclick=\"window.location.href='/{}'\">Enter chat </button>
        </li>
        ",
        chat,
        chat));

    }

    html.push_str(&*format!("
    <form action=\"/\" method=\"POST\">
        <input type=\"text\" name=\"new_chat\">
        <button type=\"submit\"> Create new chat room </button>
    </form>
    "));

    html.push_str("
        </body>
        </html>
    ");
    
    html
}

/*
    ---------------------------------------------------------------------------------------
    you will now enter the database part
    where everything related to the database will take place here

    
*/

#[derive(Debug)]
struct ChatMessage {
    id: usize,
    name: String,
    color: String,
    message: String,
}

fn insert_message(conn: &Connection, name: &str, color: &str, message: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO messages (name, color, message) VALUES ( ?1, ?2, ?3 )",
        params![name, color, message],
    )?;

    Ok(())
}

fn get_messages(conn: &Connection) -> Result<Vec<ChatMessage>> {
    let mut stmt = conn.prepare("SELECT id, name, color, message FROM messages")?;
    let messages = stmt.query_map([], |row| {
        Ok(ChatMessage{
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            message: row.get(3)?,
        })
    })?
    .collect::<Result<Vec<_>>>()?;

    Ok(messages)
}

//make a new chat room
fn setup_chat(name: String) -> Result<()> {
    // Open or create the database file
    let conn = Connection::open(format!("chats/{}.db", name))?;

    // Create a table for chat messages
    conn.execute(
        "CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            color TEXT NOT NULL,
            message TEXT NOT NULL
        )",
        [],
    )?;

    Ok(())
}



#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    // fn it_works() {
    //     let result = add(2, 2);
    //     assert_eq!(result, 4);
    // }
}
