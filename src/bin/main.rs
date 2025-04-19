use std::net::{TcpListener, TcpStream};
use chat::*;

fn main() {
    //here we only start and send the requests to the library
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();

    for stream in listener.incoming(){

        println!("stream ={:?}", stream);
        let stream = stream.unwrap();
        
        handle_connection(stream);
        
    }

    println!("Shutting down the server");
}
