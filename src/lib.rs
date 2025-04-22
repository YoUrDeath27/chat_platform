use htmlescape::decode_html;
use memchr::memmem;
use percent_encoding::percent_decode_str;
use std::{
    fs,
    path::{Path, PathBuf},
    io::{prelude::*, Read, Write},
    net::{TcpListener, TcpStream},
};

// use lazy_static::lazy_static;
use zip::write::SimpleFileOptions;
use walkdir::WalkDir;

use encoding::all::WINDOWS_1252;
use encoding::{DecoderTrap, Encoding};

use rusqlite::{params, Result, Connection};

use serde::{Serialize, Deserialize};
use serde_json;

use std::{
    sync::{mpsc, Arc, Mutex},
    thread
};

// use std::time::Duration;
// use std::path::Path;

use bcrypt::{hash, verify};

/*
--------------------------------------------------------------------------------
    every successfull response will go to response()
    every time smth fails will go to error()  <- implement error handling

    >try to make it so that it checks at every request if the user is authenticated
    implement image and video, then learn how to view it

    now the send message and send_file_message dont wor anymore in the javascript part of the website

--------------------------------------------------------------------------------
*/

pub fn handle_connection(mut stream: TcpStream){

    fs::create_dir_all("chats").unwrap(); // Create uploads directory
    fs::create_dir_all("users").unwrap();

    let conn = Connection::open("users/users.db").unwrap();

    setup_users(); //just making sure that these are here in any case 
    //for the first bootup yk

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

    if connected == false {

        let response = format!("{}\r\n{}", status_code, login());
        // println!("response = \n{}", response);
        respond(stream, response.to_string());

    } else if &buffer[..6] == b"GET / "{

        let user = get_user(
            &Connection::open("users/users.db").unwrap(), 
            get_from_buffer(buffer.clone(), "username")
        ).unwrap();

        let response = format!("{}\r\n{}", status_code, list(user));
        respond(stream, response.to_string());

    } else if &buffer[..13] == b"GET /messages" {

        let c = get_from_buffer(buffer.clone(), "chat_room");

        let conn = Connection::open(format!("{}", c)).unwrap();
        let messages = json_messages(&conn);
        println!("MESSAGES ARE= \n\n{:#?}\n\n\n\n", messages);
        let status = "HTTP/1.1 200 OK\r\n\r\n";
        let response = format!("{}{}", status, messages.unwrap());
        respond(stream, response);

    } else if &buffer[..16] == *b"GET /favicon.ico"{

        let response = format!("{}\r\n", status_code);
        respond(stream, response.to_string());

    }else {
        //ENTER A CHAT WITH THIS
        println!("\n\n\n\nchat whatever \n\n\n");

        println!("buffer = {}", String::from_utf8_lossy(&buffer[..]));

        let c = get_from_buffer(buffer.clone(), "chats_from_url");

        let response = format!("{}{}", 
                                    status_code,
                                    chat((percent_decode_str(c.as_str())
                                            .decode_utf8_lossy()
                                            .replace("+", " ")),
                                        &get_from_buffer(buffer, "username")
                                    ).to_string());
                                                    //CHANGE
        respond(stream, response.to_string());
    }

    
}

fn post(mut stream: TcpStream, buffer: Vec<u8>){
    println!("POST resposne identified");
    let status_code = "HTTP/1.1 200 OK\r\n\r\n";

    println!("\n\nPOST request is = {}", String::from_utf8_lossy(&buffer[..]));


    // let conn = Connection::open("chats/chat.db").unwrap();

    println!("\n\nrequest is = {}", String::from_utf8_lossy(&buffer[..]));

    if let Some(new_cha) = memmem::find(&buffer[..], b"new_chat=").map(|p| p as usize) {
        new_chat(stream, buffer, new_cha);

    } else if let Some(_) = memmem::find(&buffer[..], b"username="){
        connect(stream, buffer);

    } else if &buffer[..19] == *b"POST /enter_message"{
        //add message to chat
        println!("POST resposne not identified?");
        enter_message(stream, buffer);
        
    } else if let Some(_) = memmem::find(&buffer[..], b"remove_chat"){
        remove_chat(stream, buffer);
    } else if let Some(_) = memmem::find(&buffer[..], b"delete_chat"){
        delete_chat(stream, buffer);
    } else if let Some(_) = memmem::find(&buffer[..], b"remove_message") {
        remove_message(stream, buffer);
        // could make it so that it just deletes the message instead of reloading the while page?
        //maybe another day 19.04.2025
        
    } else if let Some(_) = memmem::find(&buffer[..], b"Logout") {
        logout(stream);
    } else if let Some(_) = memmem::find(&buffer[..], b"boundary=") {
        // file_upload(stream, buffer);
        let response = format!("{}{}", status_code, error("Trying to upload a file"));
        respond(stream, response.to_string());
    }
    // here put the upload file function
    else {
        let response = format!("{}{}", status_code, error("Was not able to identify request"));
        respond(stream, response.to_string());
    }
    
    
}

// fn check_create_chat()

fn new_chat(mut stream: TcpStream, buffer: Vec<u8>, new_chat: usize) {
    let status_code = "HTTP/1.1 200 OK\r\n\r\n";
    let new_chat = String::from_utf8_lossy(&buffer[new_chat + "new_chat=".len() ..] );
        //get users and see if they already have a chat created

    let conn = Connection::open("users/users.db").unwrap();
    let user = get_user(
        &Connection::open("users/users.db").unwrap(), 
        get_from_buffer(buffer.clone(), "username")
    ).unwrap();

    if user[0].name == get_from_buffer(buffer.clone(), "username") {
        if user[0].chat != "" {
            println!("user {} already has a chat", user[0].name);
            let response = format!("{}{}", status_code, error("You already have a chat created"));
            respond(stream, response.to_string());
            return;
        } else {
            //successfull creation of a new chat
            //create a new chat for the user
            println!("user {} does not have a chat", user[0].name);

            let conn = Connection::open("users/users.db").unwrap();
            user_created_chat(&conn, &user[0].name, &new_chat.replace("+", " ")).unwrap();

            println!("new chat = {:?}", new_chat);

            setup_chat(new_chat.replace("+", " ").trim().to_string()).unwrap();

            let user = get_user(
                &Connection::open("users/users.db").unwrap(), 
                get_from_buffer(buffer.clone(), "username")
            ).unwrap();
        
            let response = format!("{}{}", status_code, list(user));
            respond(stream, response.to_string());
        }
        
    }        
}

fn remove_chat(mut stream: TcpStream, buffer: Vec<u8>) {
    let response = "Set-Cookie: Chat_room=; Expires= Thu, 01 Jan 1970 00:00:00 GMT; Path=/; HttpOnly; SameSite=Strict";

    let user = get_user(
        &Connection::open("users/users.db").unwrap(), 
        get_from_buffer(buffer.clone(), "username")
    ).unwrap();

    let response = format!("HTTP/1.1 200 OK\r\n{}\r\n\r\n{}", response, list(user));

    respond(stream, response.to_string());

}

fn delete_chat(mut stream: TcpStream, buffer: Vec<u8>) {
    let user  = get_user(
                    &Connection::open("users/users.db").unwrap(), 
                    get_from_buffer(buffer.clone(), "username")
                ).unwrap();

    let filename = get_from_buffer(buffer.clone(), "delete_chat");
    fs::remove_file(&*format!("chats/{}.db", filename));

    user_deleted_chat(
        &Connection::open("users/users.db").unwrap(), 
        get_from_buffer(buffer.clone(), "username").as_str()
    ).unwrap();

    
    let response = format!("HTTP/1.1 200 OK\r\n\r\n{}", list(user));

    respond(stream, response.to_string());

}

fn enter_message(mut stream: TcpStream, buffer: Vec<u8>) {
    let input = get_from_buffer(buffer.clone(), "input_message");
    let user = get_from_buffer(buffer.clone(), "username");
    let color = get_from_buffer(buffer.clone(), "color");

    //insert_message(conn: &Connection, name: &str, color: &str, message: &str)

    let connection = get_from_buffer(buffer.clone(), "chat_room");
    let conn = Connection::open(connection).unwrap();

    insert_message(&conn, user.as_str(), color.as_str(), input.as_str());
    println!("\n\n\n\n\nHere i should add the new message");
    
    let response = "HTTP/1.1 200 OK\r\n\r\n";
    respond(stream, response.to_string());
}

fn remove_message(mut stream: TcpStream, buffer: Vec<u8>) {
    let chats = get_from_buffer(buffer.clone(), "chats");
    let message = get_from_buffer(buffer.clone(), "remove_message");
    let connection = get_from_buffer(buffer.clone(), "chat_room");

    println!("chat's name = {:?}", chats);
    let conn = Connection::open(connection).unwrap();

    match delete_message(&conn, message.parse::<usize>().unwrap()) {
        Ok(_) => {
            println!("Message deleted successfully");
            let response = "HTTP/1.1 200 OK\r\n";
            let response = format!("{}{}", 
                            response, 
                            chat(   
                                chats, 
                                &get_from_buffer(buffer, "username")));
            respond(stream, response.to_string());
        }
        Err(e) => {
            println!("Error deleting message: {}", e);
            let response = "HTTP/1.1 200 OK\r\n\r\n";
            let response = format!("{}{}", response, error("Failed to delete message"));
            respond(stream, response.to_string());
        }
    }

}
//----------------------------------------------------------------------------------

fn get_from_buffer(buffer:Vec<u8>, find: &str) -> String {

    if find == "username" { //works
        let data = memmem::find(&buffer[..], b"Auth=\"user-").map(|p| p as usize).unwrap();
        let data = &buffer[data + "Auth=\"user-".len() ..];
        let end = memmem::find(data, b"-token").map(|p| p as usize).unwrap();
        let data = &data[..end];
        return String::from_utf8_lossy(&data[..]).to_string();

    } else if find == "auth"{
        let data = memmem::find(&buffer[..], b"username=").map(|p| p as usize).unwrap();
        let data = &buffer[data + "username=".len()..];
        let mut stops = memmem::find(data, b"&").map(|p| p as usize).unwrap();
        let data = &data[..stops];
        return String::from_utf8_lossy(&data[..]).to_string();

    } else if find == "color" { //works
        let data = memmem::find(&buffer[..], b"Color=\"color-").map(|p| p as usize).unwrap();
        let data = &buffer[data + "Color=\"color-".len() ..];
        let end = memmem::find(data, b"-token").map(|p| p as usize).unwrap();
        let data = &data[..end];
        return String::from_utf8_lossy(&data[..]).to_string();

    } else if find == "get_color" { //works
        let data = memmem::find(&buffer[..], b"&color=%23").map(|p| p as usize).unwrap();
        let data = &buffer[data + "&color=%23".len() ..];
        return String::from_utf8_lossy(&data[..]).to_string();

    } else if find == "chat_room" {
        let data = memmem::find(&buffer[..], b"Chat_room=\"chats/").map(|p| p as usize).unwrap();
        let data = &buffer[data + "Chat_room=\"".len() ..];
        let end = memmem::find(data, b".db").map(|p| p as usize).unwrap();
        let data = &data[..end + 3];
        return String::from_utf8_lossy(&data[..]).to_string();
    } else if find == "chats"{
        //redo
        let data = memmem::find(&buffer[..], b"Chat_room=\"chats/").map(|p| p as usize).unwrap();
        let data = &buffer[data + "Chat_room=\"chats/".len() ..];
        let end = memmem::find(data, b".db").map(|p| p as usize).unwrap();
        let data = &data[..end];
        return String::from_utf8_lossy(&data[..]).to_string();
        
    }  else if find == "chats_from_url" {
        let data = memmem::find(&buffer[..], b"GET /").map(|p| p as usize).unwrap();
        let data = &buffer[data + "GET /".len() ..];
        let end = memmem::find(data, b" HTTP/1.1").map(|p| p as usize).unwrap();
        let data = &data[..end];
        return String::from_utf8_lossy(&data[..]).to_string();
    } else if find == "password" {
        let data = memmem::find(&buffer[..], b"password=").map(|p| p as usize).unwrap();
        let data = &buffer[data + "password=".len() ..];
        let end = memmem::find(data, b"&").map(|p| p as usize).unwrap();
        let data = &data[..end];

        return String::from_utf8_lossy(&data[..]).to_string();
    } 
    else if find == "remove_message" {
        // redo
        let data = memmem::find(&buffer[..], b"remove_message=").map(|p| p as usize).unwrap();
        let data = &buffer[data + "remove_message=".len() ..];

        return String::from_utf8_lossy(&data[..]).to_string();

    } else if find == "input_message" {
        let data = memmem::find(&buffer[..], b"input_message").map(|p| p as usize).unwrap();
        let data = &buffer[data + "input_message\":\"".len() ..];
        let end = memmem::find(data, b"\"").map(|p| p as usize).unwrap();
        let data = &data[..end];

        return String::from_utf8_lossy(&data[..]).to_string();
    } else if find == "delete_chat" {
        let data = memmem::find(&buffer[..], b"delete_chat=").map(|p| p as usize).unwrap();
        let data = &buffer[data + "delete_chat=".len() ..];

        return String::from_utf8_lossy(&data[..]).to_string();
    } 

    println!("Error: Not able to find the data \n\n{}", find);
    return String::from("Error: Not able to find the data");
    

    
}

// -------------------------------------------------------------------
fn connect(mut stream: TcpStream, buffer: Vec<u8>) {
    let username = get_from_buffer(buffer.clone(), "auth");
    let username = username.replace("+", " ");

    let password = get_from_buffer(buffer.clone(), "password");
    let color = get_from_buffer(buffer.clone(), "get_color"); //used only here

    // println!("username={}", username);
    // println!("password={}", password);
    // println!("color={}", color);
    // println!("\n\n\n");

    setup_users();
    let conn = Connection::open("users/users.db").unwrap();
      
    let users = get_users(&conn).unwrap();
    let status = "HTTP/1.1 200 Ok\r\n";
    let cookie1 = format!("Set-Cookie: Auth=\"user-{}-token\"; Path=/; HttpOnly; SameSite=Strict;\r\n", username);
    let cookie2 = format!("Set-Cookie: Color=\"color-{}-token\"; Path=/; HttpOnly; SameSite=Strict;\r\n", color);

    println!("users: \n{:#?};", users);
    let mut response = String::from("");
    let mut count = 0;

    let pass = password.clone();
    let hashed = hash(pass.clone(), 12).unwrap(); // DEFAULT_COST = 12
    // Store `hashed` in your database (e.g., as a VARCHAR(60))

    println!("lassword looks like: {:?}", hashed);

    for user in &users {
        println!("password got = {:?}", password);
        println!("password hashed = {:?}", user.pass);

        if user.name == username {

            //if they dont match
            if !(verify(password, &user.pass).unwrap()) {
                println!("\n\n\n\nTHEY DONT MATCHHHHHHHHHHHH");
                
                response = format!("{}{}{}Location: /\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n\r\n{}", status, cookie1, cookie2, login());
                break;
            }
            else{
                //if they match
                let user = get_user(
                    &Connection::open("users/users.db").unwrap(), 
                    get_from_buffer(buffer.clone(), "auth")
                ).unwrap();

                println!("\n\n\n\n\n\n\n\n\nTHEY MATCHHHHHHHHHHHHHH");
                response = format!("{}{}{}Location: /\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n\r\n{}", status, cookie1, cookie2, list(user));
                break;
            }
        }

        count += 1;
        
        println!("user:\n{:#?}", user);  
    }

    if count == users.len() {
        println!("\n\n\n\n\n\n\n\n\n\nInserting ");
        println!("idk what to do mannnnn");
        
        insert_user(&conn, &username, &hash(pass, 12).unwrap());
        
        let user = get_user(
            &Connection::open("users/users.db").unwrap(), 
            get_from_buffer(buffer.clone(), "auth")
        ).unwrap();

        response = format!("{}{}{}Location: /\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n\r\n{}", status, cookie1, cookie2, list(user));
    }
    
    respond(stream, response.to_string());
}

fn logout(mut stream: TcpStream) {
    let response = "HTTP/1.1 200 OK\r\nSet-Cookie:Auth=\"idk\"; Path=/; Expires= Thu, 01 Jan 1970 00:00:00 GMT; HttpOnly; SameSite=Strict\r\nSet-Cookie:Color=\"idk\"; Path=/; Expires= Thu, 01 Jan 1970 00:00:00 GMT; HttpOnly; SameSite=Strict\r\n\r\n\r\n";
    // let cookie1 = format!("Set-Cookie: Auth=\"user-{}-token\"; Path=/; HttpOnly; SameSite=Strict;\r\n", username);
    respond(stream, format!("{}{}", response, login()));
}

fn respond(mut stream: TcpStream, response: String) {
    // println!("response =\n{}", response);

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
fn chat(chat: String, user: &str) -> String {

    println!("chat = {:?}", chat);
    let mut html = String::from("<!DOCTYPE html>
    <html>
    <head>
        <title> Chat </title>
        <meta charset=\"utf-8\" />
        <head>
    <style>
        #chatForm{
            position: absolute;
            left: 0;
            bottom: 0;
            width: 99vw;
            height: 10vh;
            border: 2px dashed #76df87;
            background: linear-gradient(175deg, #02d02a,rgb(247, 204, 65))
        }
        #chatForm > input {
            width: 300px;
            height: 50px;
        }
        #chat-window {
            height: 80vh;
            width: 97vw;
            overflow-y: scroll; /* or use 'scroll' if you always want the scrollbar to be visible */
            border: 1px solid #ccc; /* Optional: Add a border for better visibility */
            padding: 10px; /* Optional: Add some padding for better spacing */
        }
        #chat-window > li:last-child {
            padding: 0 0 100px 0;
        }
        #file_upload {
            position: absolute;
            right: 480px;
            bottom: 57px;
            height: 2vh;
            border: 2px groove #11ecb9;
            background: linear-gradient(270deg, #02d02a,rgb(247, 204, 65))
        }
        #LogOut {

        }
    </style>
    <body>
        <h2>Welcome to the chat fam, enjoy :)</h2>
        <form action=\"/\" method=\"POST\">
            <input type=\"hidden\" name=\"remove_chat\">
            <button type=\"submit\"> Go back to browse the chat rooms </button> 
        </form>

        <ul id=\"chat-window\"> </ul>

        <form id=\"chatForm\" method=\"POST\">
            <input type=\"text\" placeholder=\"Enter a message to send in chat\" name=\"input_message\" id=\"inputMessage\">
            <button type=\"submit\"> Send message </button> 
        </form>

        <form id=\"file_upload\" method=\"POST\" enctype=\"multipart/form-data\">
            <input type=\"file\" name=\"file\" id=\"file_uploaded\" accept=\"image/*, video/* \">
            <button type=\"submit\">Upload to chat</button>
        </form>
    ");
    println!("chat = chats/{}.db", chat);
    let conn = Connection::open(format!("chats/{}.db", chat)).unwrap();

    // Insert a new message
    // insert_message(&conn, "adam", "#fe02aa", "HOW YOU DOINGGG").unwrap();
    // insert_message(&conn, "NIGA", "#00ff00", "Sup NIGGASSS!").unwrap();


    // Retrieve and print all messages
    let messages = get_messages(&conn).unwrap();
    
    html.push_str(&format!("
    </body>
    <script>
    const chatWindow = document.getElementById('chat-window');
    const currentUser = '{}';
    
     document.getElementById(\"chatForm\").addEventListener(\"submit\", function(event) {{
        event.preventDefault();
        const input = document.getElementById(\"inputMessage\");
        const inputMessage = input.value;
        input.value = \"\";
        send_message(inputMessage);
    }});

    async function send_message(message) {{
        console.log(message);

        const data = {{
            input_message: message
        }};

        fetch('/enter_message', {{
                    method:'POST',
                    headers: {{
                        'Content-Type': 'application/json'
                    }},
                    body: JSON.stringify(data)
            }}).then(response => {{
            console.log('response= ', response);
            if(!response.ok) {{
                throw new Error('Network response was not ok');
            }}
            return response.json();
            }})
        .then(data => {{
            console.log('Server response: ', data);
        }})
        .catch(error => {{
            console.error('Error:' , error);
        }})

        setTimeout(fetchMessage, 10);
    }}

    document.getElementById(\"file_upload\").addEventListener(\"submit\", function(event) {{
        event.preventDefault();
        const input = document.getElementById(\"file_uploaded\");
        if input.files.length > 0 {{
            send_file_message(input.files[0]);
        }}
    }});

    async function send_file_message(message) {{
        console.log(message);

        const data = new FormData();
        data.append('file', message);

        fetch('/enter_message', {{
                    method:'POST',
                    body: data
        }}).then(response => {{
        console.log('response= ', response);
        if(!response.ok) {{
            throw new Error('Network response was not ok');
        }}
        return response.json();
        }})
        .then(data => {{
            console.log('Server response: ', data);
        }})
        .catch(error => {{
            console.error('Error:' , error);
        }})

        setTimeout(fetchMessage, 10);
    }};

    function scrollToBottom() {{
        chatWindow.scrollTop = chatWindow.scrollHeight;
    }};

    async function fetchMessage() {{
        const response = await fetch('/messages');
        const messages = await response.json();
        chatWindow.innerHTML = messages.map(msg => msg.is_deleted ?
        `
        <li>
            <p style=\"color: ${{msg.color}};\"> 
                ${{msg.name}}
            </p>
            <h4> Message has been deleted</h4>
        </li>
        ` 
        : 
        `
        <li>
            <p style=\"color: ${{msg.color}};\"> 
                ${{msg.name}}
            </p>
            <h4> ${{msg.message}} </h4>
            ${{msg.name === currentUser ? 
                `<form action=\"/\" method=\"POST\">
                    <input type=\"hidden\" name=\"remove_message\" value=\"${{msg.id}}\">
                    <button type=\"submit\"> Delete message </button>
                </form>` 
                : ''}}
        </li>
        `).join('');
        scrollToBottom();
    }};

    setInterval(fetchMessage, 1000);
    fetchMessage();
    </script>
    
    </html>
", user));

    let cookie = format!("Set-Cookie: Chat_room=\"chats/{}.db\"; Path=/ ; HttpOnly; SameSite=Strict\r\n", chat);

    let html = String::from(format!("{}\r\n\r\n{}", cookie, html));
    
    html
}

fn list(user: Vec<User>) -> String {


    let mut html = String::from("<!DOCTYPE html>
    <html>
    <head>
        <title> Chat lists </title>
    <head>
    <body>
        <form action=\"/\" method=\"POST\">
            <input type=\"hidden\" name=\"Logout\">
            <button id=\"Logout\" type=\"submit\"> LogOut </button>
        </form> 
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

        println!("chat = {:?}", chat);
        println!("user = {:?}", user);
        
        if user[0].chat == chat {
            html.push_str(&*format!("
            <li>
                <h3> You are the owner of this chat </h3>
                {}
                <button  onclick=\"window.location.href='/{}'\">Enter chat </button>
                <form action=\"/\" method=\"POST\">
                    <input type=\"hidden\" name=\"delete_chat\" value=\"{}\">
                    <button type=\"submit\"> Delete chat </button>
                </form>
            </li>
            ",
            chat,
            chat,
            chat));
        } else {
            html.push_str(&*format!("
            <li>
                <h3> This is a public chat room </h3>
                {}
                <button  onclick=\"window.location.href='/{}'\">Enter chat </button>
            </li>
            ", chat
            , chat));
        }

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

fn error(s: &str) -> String {
    let mut html = String::from(format!("<!DOCTYPE html>
    <html>
    <head>
        <title> Error </title>
    </head>
    <style>
        h1, h2 {{
            color : red;
        }}

        button {{
            background: #f0f0f0;

        }}
                body {{
            background-color:#130e0e;
        }}

    </style>
    <body>
        <h1> Error </h1>
        <h2> {} </h2>
        <button onclick=\"window.location.href='/'\"> Go back to the main page </button>
    </body>
    </html>
    ", s));
    html
}


/*
    multi threading part
    the part that handles the ThreadPool and assigning a limited amount of workers;
*/

/*
pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv();

            match message {
                Ok(job) => {
                    println!("Worker {} gota a job; executing", id);
                    job();
                },
                Err(_) => {
                    println!("Worker {} disconected: shutting down", id);
                    break;
                }
            }
        });

        Worker {
            id, 
            thread: Some(thread),
        }
    }
}

type Job = Box<dyn FnOnce() + Send + 'static>;

impl ThreadPool {
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();

        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool {workers, sender: Some(sender) }
    }

    pub fn execute<F>(&self, f: F) 
    where
    F:FnOnce() + Send + 'static
    {
        let job = Box::new(f);

        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());

        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);

            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();

            }
        }
    }
}
 */

 /*
    ---------------------------------------------------------------------------------------
    you will now enter the database part
    where everything related to the database will take place here
    ---------------------------------------------------------------------------------------
*/

#[derive(Debug, Serialize)]
struct ChatMessage {
    id: usize,
    name: String,
    color: String,
    message: String,
    is_deleted: i8,
}

#[derive(Debug, Serialize)]
struct User {
    id: usize,
    name: String,
    pass: String,
    chat: String, // this is the chat that the user created
}

fn insert_message(conn: &Connection, name: &str, color: &str, message: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO messages (name, color, message) VALUES ( ?1, ?2, ?3 )",
        params![name, color, message],
    )?;

    Ok(())
}

fn get_messages(conn: &Connection) -> Result<Vec<ChatMessage>> {
    let mut stmt = conn.prepare("SELECT id, name, color, message, is_deleted FROM messages")?;
    let messages = stmt.query_map([], |row| {
        Ok(ChatMessage{
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            message: row.get(3)?,
            is_deleted: row.get(4)?,
        })
    })?
    .collect::<Result<Vec<_>>>()?;

    Ok(messages)
}

fn json_messages(conn: &Connection) -> Result<String, rusqlite::Error>  {
    let messages = get_messages(conn)?;
    let json = serde_json::to_string(&messages).map_err(|e| rusqlite::Error::InvalidQuery)?;
    Ok(json)
}

fn delete_message(conn: &Connection, id: usize) -> Result<()> {
    conn.execute(
        "UPDATE messages SET is_deleted = 1 WHERE id = ?1",
        params![id],
    )?;

    Ok(())
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
            message TEXT NOT NULL,
            is_deleted INTEFER DEFAULT 0
        )",
        [],
    )?;

    Ok(())
}

fn setup_users() -> Result<()> {
    // Open or create the database file
    let conn = Connection::open("users/users.db")?;
    println!("connection= {:?}", conn);

    // Create a table for chat messages
    conn.execute(
        "CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY,
            user TEXT NOT NULL,
            pass TEXT NOT NULL,
            chat TEXT DEFAULT \"\"
        )",
        [],
    )?;

    Ok(())
}

fn insert_user(conn: &Connection, name: &str, pass: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO users (user, pass) VALUES ( ?1, ?2 )",
        params![name, pass],
    )?;

    Ok(())
}

fn user_created_chat(conn: &Connection, name: &str, chat: &str) -> Result<()> {
    conn.execute(
        "UPDATE users SET chat = ?1 WHERE user = ?2",
        params![chat, name],
    )?;
    Ok(())
}

fn user_deleted_chat(conn: &Connection, name: &str) -> Result<()> {
    conn.execute(
        "UPDATE users SET chat = \"\" WHERE user = ?1",
        params![name],
    )?;
    Ok(())
}


fn get_users(conn: &Connection) -> Result<Vec<User>> {
    let mut stmt = conn.prepare("SELECT id, user, pass, chat FROM users")?;
    let messages = stmt.query_map([], |row| {
        Ok(User{
            id: row.get(0)?,
            name: row.get(1)?,
            pass: row.get(2)?,
            chat: row.get(3)?,
        })
    })?
    .collect::<Result<Vec<_>>>()?;

    Ok(messages)
}

fn get_user(conn: &Connection, username: String) -> Result<Vec<User>> {
    let mut stmt = conn.prepare("SELECT id, user, pass, chat FROM users WHERE user = ?1")?;
    let messages = stmt.query_map(params![username], |row| {
        Ok(User{
            id: row.get(0)?,
            name: row.get(1)?,
            pass: row.get(2)?,
            chat: row.get(3)?,
        })
    })?
    .collect::<Result<Vec<_>>>()?;

    Ok(messages)
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
